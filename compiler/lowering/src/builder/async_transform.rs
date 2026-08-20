use mir::{
    BasicBlock, BlockId, Function, Inst, Place, RValue, Terminator, Value,
};

pub fn lower_async_to_poll(
    original_mir: &Function,
    context_name: &str,
    _temp_count: usize,
) -> Function {
    let mut poll_func = Function::new(
        format!("{}_poll", original_mir.name),
        vec!["_ctx".to_string(), "_waker".to_string()],
        std::collections::HashSet::new(),
        false, // Returns i32 natively via FFI, no reference return
    );

    let mut state_counter = 1;
    let mut new_blocks = Vec::new();
    let mut switch_cases = Vec::new();

    // Block 0: The Dispatcher
    let mut dispatch_block = BasicBlock::new(BlockId(999999));
    let state_place = Place::Temp(1);
    
    // In our simplified codegen approach, we expect Cranelift translator 
    // to automatically map `Place::Temp(X)` to local variables,
    // and handle the prologue loading from the context struct before Block 0.
    // So we just need to provide the switch cases to the Cranelift backend.
    
    // The waker is passed as the second argument: `_waker`
    let waker_place = Place::Var("_waker".to_string());
    
    // For each block in original_mir, we split it on `Await`
    for block in &original_mir.blocks {
        let mut current_block_id = block.id;
        let mut current_insts = Vec::new();

        for inst in &block.instructions {
            match inst {
                Inst::Assign(target, RValue::Await(task_val)) => {
                    // Split point
                    let resume_block_id = BlockId(original_mir.blocks.len() + new_blocks.len() + 1);
                    switch_cases.push((state_counter, resume_block_id));

                    // Current block: Register waker, save state and return Pending
                    // (Codegen translator will insert the actual save instructions)
                    let waker_inst = Inst::RegisterWaker(task_val.clone(), Value::Place(waker_place.clone()));
                    current_insts.push(waker_inst);
                    
                    let mut pre_await_block = BasicBlock::new(current_block_id);
                    pre_await_block.instructions = current_insts;
                    pre_await_block.terminator = Some(Terminator::Return(Some(Value::Int(state_counter as i64)))); // State to save
                    new_blocks.push(pre_await_block);

                    // Setup for resume block
                    current_block_id = resume_block_id;
                    current_insts = Vec::new();
                    
                    // First instruction in resume block is to get the task result
                    let get_result_inst = Inst::Assign(target.clone(), RValue::GetTaskResult(task_val.clone()));
                    current_insts.push(get_result_inst);
                    
                    state_counter += 1;
                }
                _ => current_insts.push(inst.clone()),
            }
        }

        let mut final_block = BasicBlock::new(current_block_id);
        final_block.instructions = current_insts;
        
        // Handle Return
        if let Some(Terminator::Return(opt_val)) = &block.terminator {
            if let Some(val) = opt_val {
                let save_result = Inst::SetProperty(
                    Value::Place(Place::Temp(0)), // ctx_place
                    "_result".to_string(),
                    context_name.to_string(),
                    val.clone(),
                    false,
                );
                final_block.instructions.push(save_result);
            }
            final_block.terminator = Some(Terminator::Return(Some(Value::Int(1)))); // Ready
        } else {
            final_block.terminator = block.terminator.clone();
        }
        
        new_blocks.push(final_block);
    }

    // Set up dispatcher terminator (will be intercepted by Cranelift)
    dispatch_block.terminator = Some(Terminator::Switch {
        cond: Value::Place(state_place),
        cases: switch_cases,
        default: Some(original_mir.blocks[0].id),
    });
    
    poll_func.blocks.push(dispatch_block);
    poll_func.blocks.extend(new_blocks);

    poll_func
}
