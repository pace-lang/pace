use crate::context::CodegenContext;
use cranelift::prelude::*;
use cranelift_jit::JITModule;
use cranelift_module::{Linkage, Module};
use pace_mir::{Constant, MirBody, MirProgram, Operand, Rvalue, Statement, Terminator};

pub fn compile_mir_program(
    context: &mut CodegenContext<JITModule>,
    builder_context: &mut cranelift::prelude::FunctionBuilderContext,
    ctx: &mut cranelift::codegen::Context,
    program: &MirProgram,
) -> Result<(), crate::layouts::CodegenError> {
    
    // Pass 1: Declare all functions
    for (name, body) in &program.functions {
        let mut sig = context.module.make_signature();
        sig.call_conv = cranelift::prelude::isa::CallConv::Fast;
        
        // Return type (always I64 for now)
        sig.returns.push(AbiParam::new(types::I64));
        
        // Arguments
        for _ in 0..body.arg_count {
            sig.params.push(AbiParam::new(types::I64));
        }

        let id = context
            .module
            .declare_function(name.as_str(), Linkage::Local, &sig)
            .map_err(|e| crate::layouts::CodegenError {
                message: e.to_string(),
            })?;
        
        context.funcs.insert(*name, id);
    }

    // Pass 2: Define all functions
    for (name, body) in &program.functions {
        let id = *context.funcs.get(name).unwrap();
        compile_mir_function(context, builder_context, ctx, body, id)?;
    }

    Ok(())
}

fn compile_mir_function(
    context: &mut CodegenContext<JITModule>,
    builder_context: &mut cranelift::prelude::FunctionBuilderContext,
    ctx: &mut cranelift::codegen::Context,
    body: &MirBody,
    func_id: cranelift_module::FuncId,
) -> Result<(), crate::layouts::CodegenError> {
    ctx.func.clear();
    
    ctx.func.signature.returns.push(AbiParam::new(types::I64));
    for _ in 0..body.arg_count {
        ctx.func.signature.params.push(AbiParam::new(types::I64));
    }

    let mut builder = FunctionBuilder::new(&mut ctx.func, builder_context);
    
    // Create Cranelift blocks for all MIR BasicBlocks
    let mut blocks = Vec::with_capacity(body.basic_blocks.len());
    for _ in 0..body.basic_blocks.len() {
        blocks.push(builder.create_block());
    }
    
    // Create Cranelift Variables for all Locals
    let mut locals = Vec::with_capacity(body.local_decls.len());
    for _ in 0..body.local_decls.len() {
        let var = builder.declare_var(types::I64);
        locals.push(var);
    }

    builder.append_block_params_for_function_params(blocks[0]);
    builder.switch_to_block(blocks[0]);

    // Map arguments to local variables
    for i in 0..body.arg_count {
        let param_val = builder.block_params(blocks[0])[i];
        builder.def_var(locals[i + 1], param_val); // Arg 0 is Local 1, etc.
    }

    // Translate blocks
    for (bb_idx, bb_data) in body.basic_blocks.iter().enumerate() {
        let current_block = blocks[bb_idx];
        
        if builder.current_block() != Some(current_block) {
            builder.switch_to_block(current_block);
        }

        // Statements
        for stmt in &bb_data.statements {
            match stmt {
                Statement::Assign(place, rvalue) => {
                    let val = translate_rvalue(&mut builder, context, rvalue, &locals)?;
                    store_to_place(&mut builder, context, place, &locals, val)?;
                }
                _ => {}
            }
        }

        // Terminator
        if let Some(terminator) = &bb_data.terminator {
            match terminator {
                Terminator::Goto { target } => {
                    builder.ins().jump(blocks[target.index()], &[]);
                }
                Terminator::Return => {
                    // Return value is always in Local 0
                    let ret_val = builder.use_var(locals[0]);
                    builder.ins().return_(&[ret_val]);
                }
                Terminator::SwitchInt { discr, targets } => {
                    let discr_val = translate_operand(&mut builder, context, discr, &locals)?;
                    
                    // A simple switch logic (if false, go to false target, else true target)
                    // Currently SwitchTargets only has 2 targets (false, true) in our MIR Builder
                    // We'll just branch if zero (false)
                    let false_block = blocks[targets.targets[0].index()];
                    let true_block = blocks[targets.targets[1].index()];
                    
                    builder.ins().brif(discr_val, true_block, &[], false_block, &[]);
                }
                Terminator::Call { func, args, destination, target, .. } => {
                    if let Operand::Constant(Constant::Function(func_name)) = func {
                        if let Some(callee_id) = context.funcs.get(func_name) {
                            let callee_ref = context.module.declare_func_in_func(*callee_id, builder.func);
                            let mut arg_vals = Vec::new();
                            for arg in args {
                                arg_vals.push(translate_operand(&mut builder, context, arg, &locals)?);
                            }
                            
                            let inst = builder.ins().call(callee_ref, &arg_vals);
                            let result_val = builder.inst_results(inst)[0];
                            builder.def_var(locals[destination.local.index()], result_val);
                            
                            if let Some(next_block) = target {
                                builder.ins().jump(blocks[next_block.index()], &[]);
                            }
                        } else {
                            // Missing function
                        }
                    }
                }
                Terminator::Unreachable => {
                    builder.ins().trap(cranelift::prelude::TrapCode::user(1).unwrap());
                }
            }
        }
    }

    builder.seal_all_blocks();
    builder.finalize(context.module.target_config());

    context
        .module
        .define_function(func_id, ctx)
        .map_err(|e| crate::layouts::CodegenError {
            message: e.to_string(),
        })?;
    
    Ok(())
}

fn translate_rvalue(
    builder: &mut FunctionBuilder,
    context: &mut CodegenContext<JITModule>,
    rvalue: &Rvalue,
    locals: &[Variable],
) -> Result<Value, crate::layouts::CodegenError> {
    match rvalue {
        Rvalue::Use(operand) => translate_operand(builder, context, operand, locals),
        Rvalue::BinaryOp(op, left, right) => {
            let lhs = translate_operand(builder, context, left, locals)?;
            let rhs = translate_operand(builder, context, right, locals)?;
            
            use pace_ast::BinaryOp;
            let val = match op {
                BinaryOp::Add => builder.ins().iadd(lhs, rhs),
                BinaryOp::Sub => builder.ins().isub(lhs, rhs),
                BinaryOp::Mul => builder.ins().imul(lhs, rhs),
                BinaryOp::Div => builder.ins().sdiv(lhs, rhs),
                BinaryOp::Mod => builder.ins().srem(lhs, rhs),
                BinaryOp::Eq => {
                    let b = builder.ins().icmp(cranelift::codegen::ir::condcodes::IntCC::Equal, lhs, rhs);
                    builder.ins().uextend(types::I64, b)
                }
                BinaryOp::NotEq => {
                    let b = builder.ins().icmp(cranelift::codegen::ir::condcodes::IntCC::NotEqual, lhs, rhs);
                    builder.ins().uextend(types::I64, b)
                }
                BinaryOp::Less => {
                    let b = builder.ins().icmp(cranelift::codegen::ir::condcodes::IntCC::SignedLessThan, lhs, rhs);
                    builder.ins().uextend(types::I64, b)
                }
                BinaryOp::Greater => {
                    let b = builder.ins().icmp(cranelift::codegen::ir::condcodes::IntCC::SignedGreaterThan, lhs, rhs);
                    builder.ins().uextend(types::I64, b)
                }
                BinaryOp::LessEq => {
                    let b = builder.ins().icmp(cranelift::codegen::ir::condcodes::IntCC::SignedLessThanOrEqual, lhs, rhs);
                    builder.ins().uextend(types::I64, b)
                }
                BinaryOp::GreaterEq => {
                    let b = builder.ins().icmp(cranelift::codegen::ir::condcodes::IntCC::SignedGreaterThanOrEqual, lhs, rhs);
                    builder.ins().uextend(types::I64, b)
                }
                _ => builder.ins().iconst(types::I64, 0), // Fallback
            };
            Ok(val)
        }
        Rvalue::Aggregate(pace_mir::AggregateKind::Class(class_name), operands) => {
            let layout = context.class_layouts.get(class_name).unwrap();
            let size = 16 + layout.fields.len() * 8;
            let size_val = builder.ins().iconst(types::I64, size as i64);

            let malloc_id = *context.funcs.get(&ustr::Ustr::from("malloc")).unwrap();
            let local_malloc = context.module.declare_func_in_func(malloc_id, builder.func);
            let call = builder.ins().call(local_malloc, &[size_val]);
            let obj_ptr = builder.inst_results(call)[0];

            let one = builder.ins().iconst(types::I64, 1);
            builder.ins().store(cranelift::prelude::MemFlagsData::new(), one, obj_ptr, 0);

            let vtable_gv = context.module.declare_data_in_func(layout.vtable_id, builder.func);
            let ptr_ty = context.module.target_config().pointer_type();
            let vtable_addr = builder.ins().symbol_value(ptr_ty, vtable_gv);
            builder.ins().store(cranelift::prelude::MemFlagsData::new(), vtable_addr, obj_ptr, 8);
            
            let zero = builder.ins().iconst(types::I64, 0);
            for (field_name, &(offset, _)) in &layout.fields {
                if field_name == "__mailbox" {
                    let mb_create_id = *context.funcs.get(&ustr::Ustr::from("__pace_mailbox_create")).unwrap();
                    let local_mb = context.module.declare_func_in_func(mb_create_id, builder.func);
                    let mb_call = builder.ins().call(local_mb, &[]);
                    let mb_ptr = builder.inst_results(mb_call)[0];
                    builder.ins().store(cranelift::prelude::MemFlagsData::new(), mb_ptr, obj_ptr, offset as i32);
                } else {
                    builder.ins().store(cranelift::prelude::MemFlagsData::new(), zero, obj_ptr, offset as i32);
                }
            }
            
            let init_name = format!("{}_init", class_name);
            if let Some(&init_id) = context.funcs.get(&ustr::Ustr::from(&init_name)) {
                let local_init = context.module.declare_func_in_func(init_id, builder.func);
                let mut arg_vals = vec![obj_ptr];
                for op in operands {
                    arg_vals.push(translate_operand(builder, context, op, locals)?);
                }
                builder.ins().call(local_init, &arg_vals);
            }
            
            Ok(obj_ptr)
        }
        _ => Ok(builder.ins().iconst(types::I64, 0)),
    }
}

fn translate_operand(
    builder: &mut FunctionBuilder,
    context: &mut CodegenContext<JITModule>,
    operand: &Operand,
    locals: &[Variable],
) -> Result<Value, crate::layouts::CodegenError> {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => {
            translate_place(builder, context, place, locals)
        }
        Operand::Constant(c) => match c {
            Constant::Int(i) => Ok(builder.ins().iconst(types::I64, *i)),
            Constant::Bool(b) => Ok(builder.ins().iconst(types::I64, if *b { 1 } else { 0 })),
            Constant::String(s) => {
                let s_ustr = ustr::Ustr::from(s);
                let string_name = if let Some(name) = context.string_cache.get(&s_ustr) {
                    name.clone()
                } else {
                    let id = context.string_id;
                    context.string_id += 1;
                    let name = format!("__string_{}", id);
                    context.string_cache.insert(s_ustr.clone(), name.clone());

                    let mut data_ctx = cranelift_module::DataDescription::new();
                    let bytes = s.as_bytes();
                    data_ctx.define(bytes.to_vec().into_boxed_slice());

                    let data_id = context
                        .module
                        .declare_data(&name, cranelift_module::Linkage::Local, true, false)
                        .unwrap();

                    context.module.define_data(data_id, &data_ctx).unwrap();
                    name
                };

                let data_id = context
                    .module
                    .declare_data(&string_name, cranelift_module::Linkage::Local, true, false)
                    .unwrap();

                let ptr_ty = context.module.target_config().pointer_type();
                let local_data = context
                    .module
                    .declare_data_in_func(data_id, builder.func);
                Ok(builder.ins().symbol_value(ptr_ty, local_data))
            }
            _ => Ok(builder.ins().iconst(types::I64, 0)),
        },
    }
}

fn translate_place(
    builder: &mut FunctionBuilder,
    context: &mut CodegenContext<JITModule>,
    place: &pace_mir::Place,
    locals: &[Variable],
) -> Result<Value, crate::layouts::CodegenError> {
    let mut val = builder.use_var(locals[place.local.index()]);
    for proj in &place.projection {
        match proj {
            pace_mir::ProjectionElem::Field(prop, class_name) => {
                if let Some(layout) = context.class_layouts.get(class_name) {
                    if let Some(&(offset, _)) = layout.fields.get(prop) {
                        val = builder.ins().load(types::I64, cranelift::prelude::MemFlagsData::new(), val, offset as i32);
                    }
                }
            }
            _ => {}
        }
    }
    Ok(val)
}

fn store_to_place(
    builder: &mut FunctionBuilder,
    context: &mut CodegenContext<JITModule>,
    place: &pace_mir::Place,
    locals: &[Variable],
    value: Value,
) -> Result<(), crate::layouts::CodegenError> {
    if place.projection.is_empty() {
        builder.def_var(locals[place.local.index()], value);
    } else {
        let mut ptr = builder.use_var(locals[place.local.index()]);
        for (i, proj) in place.projection.iter().enumerate() {
            if i == place.projection.len() - 1 {
                if let pace_mir::ProjectionElem::Field(prop, class_name) = proj {
                    if let Some(layout) = context.class_layouts.get(class_name) {
                        if let Some(&(offset, _)) = layout.fields.get(prop) {
                            builder.ins().store(cranelift::prelude::MemFlagsData::new(), value, ptr, offset as i32);
                        }
                    }
                }
            } else {
                if let pace_mir::ProjectionElem::Field(prop, class_name) = proj {
                    if let Some(layout) = context.class_layouts.get(class_name) {
                        if let Some(&(offset, _)) = layout.fields.get(prop) {
                            ptr = builder.ins().load(types::I64, cranelift::prelude::MemFlagsData::new(), ptr, offset as i32);
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
