use crate::{Constant, MirBody, Operand, Rvalue, Statement, Terminator, UnaryOp};
use pace_ast::BinaryOp;
use std::collections::HashMap;

pub fn optimize(body: &mut MirBody) -> bool {
    let mut changed = false;
    
    // Forward pass through blocks
    for block in &mut body.basic_blocks {
        // Maps a Local to its known Constant value within a single basic block.
        let mut known_constants: HashMap<crate::PlaceBase, Constant> = HashMap::new();
        
        for stmt in &mut block.statements {
            match stmt {
                Statement::Assign(place, rvalue) => {
                    // Try to fold the rvalue
                    if let Some(folded) = fold_rvalue(rvalue, &known_constants) {
                        *rvalue = Rvalue::Use(Operand::Constant(folded.clone()));
                        changed = true;
                        
                        // If this place is just a single local, remember its value
                        if place.projection.is_empty() {
                            known_constants.insert(place.base.clone(), folded);
                        }
                    } else if let Rvalue::Use(Operand::Constant(c)) = rvalue {
                        // Already a constant, just remember it if it's a simple local
                        if place.projection.is_empty() {
                            known_constants.insert(place.base.clone(), c.clone());
                        }
                    } else if place.projection.is_empty() {
                        // If it's not a constant, remove any known constant for this local
                        // since it has been reassigned. (A more advanced pass would use SSA).
                        known_constants.remove(&place.base);
                    }
                }
                Statement::FakeRead(_) => {}
            }
        }
        
        // Optimize terminators
        if let Some(terminator) = &mut block.terminator {
            match terminator {
                Terminator::SwitchInt { discr, targets } => {
                    // If the discriminator is a known constant or we can resolve it
                    let val = match discr {
                        Operand::Constant(Constant::Int(i)) => Some(*i),
                        Operand::Constant(Constant::Bool(b)) => Some(if *b { 1 } else { 0 }),
                        Operand::Copy(p) | Operand::Move(p) => {
                            if p.projection.is_empty() {
                                if let Some(c) = known_constants.get(&p.base) {
                                    match c {
                                        Constant::Int(i) => Some(*i),
                                        Constant::Bool(b) => Some(if *b { 1 } else { 0 }),
                                        _ => None
                                    }
                                } else { None }
                            } else { None }
                        }
                        _ => None
                    };
                    
                    if let Some(v) = val {
                        // We can resolve the branch statically!
                        let target_block = targets.target_for_value(v);
                        *terminator = Terminator::Goto { target: target_block };
                        changed = true;
                    }
                }
                _ => {}
            }
        }
    }
    
    changed
}

fn fold_rvalue(rvalue: &mut Rvalue, constants: &HashMap<crate::PlaceBase, Constant>) -> Option<Constant> {
    match rvalue {
        Rvalue::Use(op) => resolve_operand(op, constants),
        Rvalue::BinaryOp(op, left, right) => {
            let l_const = resolve_operand(left, constants)?;
            let r_const = resolve_operand(right, constants)?;
            
            match (l_const, r_const) {
                (Constant::Int(l), Constant::Int(r)) => {
                    match op {
                        BinaryOp::Add => Some(Constant::Int(l.wrapping_add(r))),
                        BinaryOp::Sub => Some(Constant::Int(l.wrapping_sub(r))),
                        BinaryOp::Mul => Some(Constant::Int(l.wrapping_mul(r))),
                        BinaryOp::Div => if r != 0 { Some(Constant::Int(l / r)) } else { None },
                        BinaryOp::Mod => if r != 0 { Some(Constant::Int(l % r)) } else { None },
                        BinaryOp::Eq => Some(Constant::Bool(l == r)),
                        BinaryOp::NotEq => Some(Constant::Bool(l != r)),
                        BinaryOp::Less => Some(Constant::Bool(l < r)),
                        BinaryOp::LessEq => Some(Constant::Bool(l <= r)),
                        BinaryOp::Greater => Some(Constant::Bool(l > r)),
                        BinaryOp::GreaterEq => Some(Constant::Bool(l >= r)),
                        _ => None
                    }
                }
                (Constant::Float(l), Constant::Float(r)) => {
                    match op {
                        BinaryOp::Add => Some(Constant::Float(l + r)),
                        BinaryOp::Sub => Some(Constant::Float(l - r)),
                        BinaryOp::Mul => Some(Constant::Float(l * r)),
                        BinaryOp::Div => Some(Constant::Float(l / r)),
                        BinaryOp::Eq => Some(Constant::Bool(l == r)),
                        BinaryOp::NotEq => Some(Constant::Bool(l != r)),
                        BinaryOp::Less => Some(Constant::Bool(l < r)),
                        BinaryOp::LessEq => Some(Constant::Bool(l <= r)),
                        BinaryOp::Greater => Some(Constant::Bool(l > r)),
                        BinaryOp::GreaterEq => Some(Constant::Bool(l >= r)),
                        _ => None
                    }
                }
                (Constant::Bool(l), Constant::Bool(r)) => {
                    match op {
                        BinaryOp::Eq => Some(Constant::Bool(l == r)),
                        BinaryOp::NotEq => Some(Constant::Bool(l != r)),
                        BinaryOp::And => Some(Constant::Bool(l && r)),
                        BinaryOp::Or => Some(Constant::Bool(l || r)),
                        _ => None
                    }
                }
                _ => None
            }
        }
        Rvalue::UnaryOp(op, operand) => {
            let c = resolve_operand(operand, constants)?;
            match c {
                Constant::Int(i) => match op {
                    UnaryOp::Neg => Some(Constant::Int(-i)),
                    UnaryOp::Not => Some(Constant::Int(!i)), // Bitwise not for ints? Pace might not have it.
                },
                Constant::Float(f) => match op {
                    UnaryOp::Neg => Some(Constant::Float(-f)),
                    UnaryOp::Not => None,
                },
                Constant::Bool(b) => match op {
                    UnaryOp::Not => Some(Constant::Bool(!b)),
                    UnaryOp::Neg => None,
                },
                _ => None
            }
        }
        _ => None
    }
}

fn resolve_operand(operand: &mut Operand, constants: &HashMap<crate::PlaceBase, Constant>) -> Option<Constant> {
    match operand {
        Operand::Constant(c) => Some(c.clone()),
        Operand::Copy(p) | Operand::Move(p) => {
            if p.projection.is_empty() {
                if let Some(c) = constants.get(&p.base) {
                    // Modify the operand in place to avoid future lookups
                    *operand = Operand::Constant(c.clone());
                    return Some(c.clone());
                }
            }
            None
        }
    }
}
