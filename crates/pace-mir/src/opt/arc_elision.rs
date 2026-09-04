use crate::{BasicBlock, Constant, MirBody, Operand, Terminator};
use ustr::Ustr;

pub fn optimize(body: &mut MirBody) -> bool {
    let mut changed = false;
    
    let retain_func = Ustr::from("__pace_retain");
    let release_func = Ustr::from("__pace_release");

    // 1. Calculate predecessor counts for each basic block
    let mut pred_counts = vec![0; body.basic_blocks.len()];
    for block in &body.basic_blocks {
        if let Some(term) = &block.terminator {
            match term {
                Terminator::Goto { target } => pred_counts[target.0] += 1,
                Terminator::SwitchInt { targets, .. } => {
                    for target in targets.all_targets() {
                        pred_counts[target.0] += 1;
                    }
                }
                Terminator::Call { target, cleanup, .. } => {
                    if let Some(t) = target { pred_counts[t.0] += 1; }
                    if let Some(c) = cleanup { pred_counts[c.0] += 1; }
                }
                Terminator::Return | Terminator::Unreachable => {}
            }
        }
    }

    let mut modifications = Vec::new();

    // 2. Find retain -> release chains
    for (bb1_idx, block1) in body.basic_blocks.iter().enumerate() {
        if let Some(Terminator::Call { func: func1, args: args1, target: Some(bb2_idx), .. }) = &block1.terminator {
            if is_func_call(func1, retain_func) && args1.len() == 1 {
                let retained_arg = &args1[0];
                
                // Ensure BB2 is only reached from BB1
                if pred_counts[bb2_idx.0] != 1 {
                    continue;
                }
                
                let bb2 = &body.basic_blocks[bb2_idx.0];
                
                // For now, we only optimize if BB2's statements are empty or just assignments to unused temporaries.
                // Actually, if BB2 is just a passthrough to `release`, any statements it has are safe 
                // as long as they don't mutate the retained_arg.
                // Let's assume for simplicity it's safe if it doesn't mutate the local.
                let mut is_safe = true;
                for stmt in &bb2.statements {
                    if let crate::Statement::Assign(place, _) = stmt {
                        if let Operand::Copy(ret_place) | Operand::Move(ret_place) = retained_arg {
                            if place.base == ret_place.base {
                                is_safe = false;
                                break;
                            }
                        }
                    }
                }
                
                if is_safe {
                    if let Some(Terminator::Call { func: func2, args: args2, target: bb3_target, .. }) = &bb2.terminator {
                        if is_func_call(func2, release_func) && args2.len() == 1 {
                            let released_arg = &args2[0];
                            
                            if retained_arg == released_arg {
                                modifications.push((BasicBlock(bb1_idx), *bb2_idx, *bb3_target));
                            }
                        }
                    }
                }
            }
        }
    }

    // 3. Apply modifications
    for (bb1, bb2, bb3_target) in modifications {
        body.basic_blocks[bb1.0].terminator = Some(Terminator::Goto { target: bb2 });
        if let Some(bb3) = bb3_target {
            body.basic_blocks[bb2.0].terminator = Some(Terminator::Goto { target: bb3 });
        } else {
            body.basic_blocks[bb2.0].terminator = Some(Terminator::Unreachable);
        }
        changed = true;
    }

    changed
}

fn is_func_call(op: &Operand, expected_name: Ustr) -> bool {
    if let Operand::Constant(Constant::Function(name)) = op {
        *name == expected_name
    } else {
        false
    }
}
