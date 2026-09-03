use crate::{BasicBlock, Local, MirBody, Statement, Terminator, Operand, Rvalue};
use std::collections::HashSet;

pub fn optimize(body: &mut MirBody) -> bool {
    let mut changed = false;
    
    // 1. Compute Liveness and remove dead assignments
    let mut used_locals = HashSet::new();
    
    // Always mark arguments and return pointer as used
    used_locals.insert(Local(0));
    for i in 1..=body.arg_count {
        used_locals.insert(Local(i));
    }
    
    // Also, if a local is used as the receiver of a method call or passed as a ref,
    // we need to be careful. Let's do a simple pass to find all used locals.
    for block in &body.basic_blocks {
        for stmt in &block.statements {
            match stmt {
                Statement::Assign(place, rvalue) => {
                    // Mark anything in projection as used (e.g. `a.b = ...` means `a` is used)
                    if !place.projection.is_empty() {
                        used_locals.insert(place.local);
                    }
                    // Mark RHS as used
                    mark_rvalue_uses(rvalue, &mut used_locals);
                }
                Statement::FakeRead(place) => {
                    used_locals.insert(place.local);
                }
            }
        }
        
        if let Some(terminator) = &block.terminator {
            match terminator {
                Terminator::Goto { .. } => {}
                Terminator::SwitchInt { discr, .. } => {
                    mark_operand_uses(discr, &mut used_locals);
                }
                Terminator::Return => {}
                Terminator::Unreachable => {}
                Terminator::Call { func, args, destination, .. } => {
                    mark_operand_uses(func, &mut used_locals);
                    for arg in args {
                        mark_operand_uses(arg, &mut used_locals);
                    }
                    // For destination, if it's a projection, the base is used
                    if !destination.projection.is_empty() {
                        used_locals.insert(destination.local);
                    }
                }
            }
        }
    }
    
    // Now remove assignments to unused locals
    for block in &mut body.basic_blocks {
        let original_len = block.statements.len();
        block.statements.retain(|stmt| {
            if let Statement::Assign(place, _) = stmt {
                if place.projection.is_empty() && !used_locals.contains(&place.local) {
                    // Assignment to a local that is never used!
                    return false;
                }
            }
            true
        });
        if block.statements.len() != original_len {
            changed = true;
        }
    }
    
    // 2. Remove Unreachable Blocks
    let mut reachable_blocks = HashSet::new();
    let mut worklist = vec![BasicBlock(0)];
    reachable_blocks.insert(BasicBlock(0));
    
    while let Some(bb) = worklist.pop() {
        if let Some(terminator) = &body.basic_blocks[bb.0].terminator {
            let successors = match terminator {
                Terminator::Goto { target } => vec![*target],
                Terminator::SwitchInt { targets, .. } => targets.all_targets().to_vec(),
                Terminator::Return | Terminator::Unreachable => vec![],
                Terminator::Call { target, .. } => {
                    if let Some(t) = target {
                        vec![*t]
                    } else {
                        vec![]
                    }
                }
            };
            
            for succ in successors {
                if reachable_blocks.insert(succ) {
                    worklist.push(succ);
                }
            }
        }
    }
    
    // Replace unreachable blocks with just Terminator::Unreachable
    for (i, block) in body.basic_blocks.iter_mut().enumerate() {
        if !reachable_blocks.contains(&BasicBlock(i)) {
            if block.terminator != Some(Terminator::Unreachable) {
                block.statements.clear();
                block.terminator = Some(Terminator::Unreachable);
                changed = true;
            }
        }
    }
    
    changed
}

fn mark_rvalue_uses(rvalue: &Rvalue, used_locals: &mut HashSet<Local>) {
    match rvalue {
        Rvalue::Use(op) => mark_operand_uses(op, used_locals),
        Rvalue::BinaryOp(_, left, right) => {
            mark_operand_uses(left, used_locals);
            mark_operand_uses(right, used_locals);
        }
        Rvalue::UnaryOp(_, op) => mark_operand_uses(op, used_locals),
        Rvalue::Cast(op, _) => mark_operand_uses(op, used_locals),
        Rvalue::Ref(_, place) => {
            used_locals.insert(place.local);
        }
        Rvalue::Aggregate(_, ops) => {
            for op in ops {
                mark_operand_uses(op, used_locals);
            }
        }
    }
}

fn mark_operand_uses(operand: &Operand, used_locals: &mut HashSet<Local>) {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => {
            used_locals.insert(place.local);
        }
        Operand::Constant(_) => {}
    }
}
