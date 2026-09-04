use crate::{BasicBlock, BasicBlockData, Local, MirBody, MirProgram, Operand, Place, Rvalue, Statement, Terminator};
use ustr::Ustr;

pub fn optimize(program: &mut MirProgram) -> bool {
    let mut changed = false;
    let function_names: Vec<Ustr> = program.functions.keys().copied().collect();

    for caller_name in function_names {
        let mut inlined_count: std::collections::HashMap<Ustr, usize> = std::collections::HashMap::new();
        loop {
            // Find an inline candidate
            let candidate = if let Some(caller_body) = program.functions.get(&caller_name) {
                find_inline_candidate(caller_body, program, &mut inlined_count)
            } else {
                None
            };
            
            if let Some((call_bb, callee_name)) = candidate {
                let callee_body = program.functions.get(&callee_name).unwrap().clone();
                let caller_body = program.functions.get_mut(&caller_name).unwrap();
                
                inline_function(caller_body, call_bb, &callee_body);
                changed = true;
            } else {
                break;
            }
        }
    }

    changed
}

fn find_inline_candidate(
    caller_body: &MirBody,
    program: &MirProgram,
    inlined_count: &mut std::collections::HashMap<Ustr, usize>,
) -> Option<(BasicBlock, Ustr)> {
    // Prevent exponential code bloat: stop inlining if the caller is already too large
    if caller_body.basic_blocks.len() > 100 {
        return None;
    }

    for (bb_idx, bb_data) in caller_body.basic_blocks.iter().enumerate() {
        if let Some(Terminator::Call { func, .. }) = &bb_data.terminator {
            if let Operand::Constant(crate::statement::Constant::Function(callee_name)) = func {
                // Prevent direct recursive inlining into itself
                if *callee_name == caller_body.name {
                    continue;
                }
                
                let count = inlined_count.get(callee_name).copied().unwrap_or(0);
                if count >= 3 {
                    continue;
                }
                
                if let Some(callee_body) = program.functions.get(callee_name) {
                    if is_inlinable(callee_body) {
                        *inlined_count.entry(*callee_name).or_insert(0) += 1;
                        return Some((BasicBlock(bb_idx), *callee_name));
                    }
                }
            }
        }
    }
    None
}

fn is_inlinable(callee_body: &MirBody) -> bool {
    if callee_body.is_extern {
        return false;
    }
    
    // Heuristic: only inline functions that are small enough
    if callee_body.basic_blocks.len() > 15 {
        return false;
    }
    
    true
}

fn inline_function(caller_body: &mut MirBody, call_bb: BasicBlock, callee_body: &MirBody) {
    let call_terminator = caller_body.basic_blocks[call_bb.0].terminator.take().unwrap();
    
    let (args, destination, target) = match call_terminator {
        Terminator::Call { args, destination, target, .. } => (args, destination, target),
        _ => unreachable!(),
    };
    
    let return_target = target.unwrap_or_else(|| {
        // If the call doesn't return (target is None), we create an unreachable block
        let unreachable_bb = BasicBlock(caller_body.basic_blocks.len());
        caller_body.basic_blocks.push(BasicBlockData {
            statements: vec![],
            terminator: Some(Terminator::Unreachable),
            is_cleanup: false,
        });
        unreachable_bb
    });

    let local_offset = caller_body.local_decls.len();
    for (i, decl) in callee_body.local_decls.iter().enumerate() {
        if i > 0 { // Skip the callee's return slot _0, we use the destination place instead
            caller_body.local_decls.push(decl.clone());
        }
    }

    // Map callee local `L` to caller local `M`
    let map_local = |l: Local| -> Local {
        if l.index() == 0 {
            if let crate::statement::PlaceBase::Local(loc) = destination.base {
                loc
            } else {
                panic!("Cannot inline into a static destination");
            }
        } else {
            Local(local_offset + l.index() - 1)
        }
    };

    let bb_offset = caller_body.basic_blocks.len();
    for bb_data in &callee_body.basic_blocks {
        caller_body.basic_blocks.push(BasicBlockData {
            statements: vec![], // we'll fill this in the next pass
            terminator: None,
            is_cleanup: bb_data.is_cleanup,
        });
    }

    let map_bb = |bb: BasicBlock| -> BasicBlock {
        BasicBlock(bb_offset + bb.0)
    };

    // Assign arguments to the callee's parameters in the caller's call block
    for (i, arg) in args.into_iter().enumerate() {
        let param_local = map_local(Local(i + 1));
        caller_body.basic_blocks[call_bb.0].statements.push(Statement::Assign(
            Place::new_local(param_local),
            Rvalue::Use(arg),
        ));
    }
    
    // Jump to the first block of the inlined callee
    caller_body.basic_blocks[call_bb.0].terminator = Some(Terminator::Goto { target: map_bb(BasicBlock(0)) });

    // Now copy and translate statements and terminators
    for (i, bb_data) in callee_body.basic_blocks.iter().enumerate() {
        let mut translated_stmts = Vec::with_capacity(bb_data.statements.len());
        for stmt in &bb_data.statements {
            translated_stmts.push(translate_statement(stmt, &map_local));
        }
        let translated_terminator = bb_data.terminator.as_ref().map(|t| {
            translate_terminator(
                t,
                &map_local,
                &map_bb,
                return_target,
            )
        });
        
        let caller_bb_idx = bb_offset + i;
        caller_body.basic_blocks[caller_bb_idx].statements = translated_stmts;
        caller_body.basic_blocks[caller_bb_idx].terminator = translated_terminator;
    }
}

// --- Translation helpers ---

fn translate_place(place: &Place, map_local: &impl Fn(Local) -> Local) -> Place {
    let mut new_place = place.clone();
    if let crate::statement::PlaceBase::Local(l) = place.base {
        new_place.base = crate::statement::PlaceBase::Local(map_local(l));
    }
    new_place
}

fn translate_operand(operand: &Operand, map_local: &impl Fn(Local) -> Local) -> Operand {
    match operand {
        Operand::Copy(p) => Operand::Copy(translate_place(p, map_local)),
        Operand::Move(p) => Operand::Move(translate_place(p, map_local)),
        Operand::Constant(c) => Operand::Constant(c.clone()),
    }
}

fn translate_rvalue(rvalue: &Rvalue, map_local: &impl Fn(Local) -> Local) -> Rvalue {
    match rvalue {
        Rvalue::Use(op) => Rvalue::Use(translate_operand(op, map_local)),
        Rvalue::Ref(kind, p) => Rvalue::Ref(*kind, translate_place(p, map_local)),
        Rvalue::Cast(op, ty) => Rvalue::Cast(translate_operand(op, map_local), ty.clone()),
        Rvalue::BinaryOp(op, a, b) => Rvalue::BinaryOp(
            op.clone(),
            translate_operand(a, map_local),
            translate_operand(b, map_local),
        ),
        Rvalue::UnaryOp(op, a) => Rvalue::UnaryOp(
            op.clone(),
            translate_operand(a, map_local),
        ),
        Rvalue::Aggregate(kind, ops) => {
            let mut translated_ops = Vec::with_capacity(ops.len());
            for op in ops {
                translated_ops.push(translate_operand(op, map_local));
            }
            Rvalue::Aggregate(kind.clone(), translated_ops)
        }
    }
}

fn translate_statement(stmt: &Statement, map_local: &impl Fn(Local) -> Local) -> Statement {
    match stmt {
        Statement::Assign(p, rv) => Statement::Assign(
            translate_place(p, map_local),
            translate_rvalue(rv, map_local),
        ),
        Statement::FakeRead(p) => Statement::FakeRead(translate_place(p, map_local)),
    }
}

fn translate_terminator(
    terminator: &Terminator,
    map_local: &impl Fn(Local) -> Local,
    map_bb: &impl Fn(BasicBlock) -> BasicBlock,
    return_target: BasicBlock,
) -> Terminator {
    match terminator {
        Terminator::Goto { target } => Terminator::Goto { target: map_bb(*target) },
        Terminator::Return => Terminator::Goto { target: return_target },
        Terminator::Unreachable => Terminator::Unreachable,
        Terminator::Call { func, args, destination, target, cleanup } => {
            let mut translated_args = Vec::with_capacity(args.len());
            for arg in args {
                translated_args.push(translate_operand(arg, map_local));
            }
            Terminator::Call {
                func: translate_operand(func, map_local),
                args: translated_args,
                destination: translate_place(destination, map_local),
                target: target.map(|bb| map_bb(bb)),
                cleanup: cleanup.map(|bb| map_bb(bb)),
            }
        }
        Terminator::SwitchInt { discr, targets } => {
            let mut translated_targets = Vec::with_capacity(targets.targets.len());
            for t in &targets.targets {
                translated_targets.push(map_bb(*t));
            }
            Terminator::SwitchInt {
                discr: translate_operand(discr, map_local),
                targets: crate::basic_block::SwitchTargets {
                    values: targets.values.clone(),
                    targets: translated_targets,
                },
            }
        }
    }
}
