use crate::{MirBody, Statement, Rvalue, AggregateKind, Operand};
use std::collections::HashSet;

pub fn optimize(body: &mut MirBody) -> bool {
    let mut changed = false;
    
    // Find all locals that are assigned a Class
    let mut class_locals = std::collections::HashMap::new();
    
    for (bb_idx, block) in body.basic_blocks.iter().enumerate() {
        for (stmt_idx, stmt) in block.statements.iter().enumerate() {
            if let Statement::Assign(place, Rvalue::Aggregate(AggregateKind::Class(_, _), _)) = stmt {
                if place.projection.is_empty() {
                    class_locals.insert(place.base.clone(), (bb_idx, stmt_idx));
                }
            }
        }
    }
    
    if class_locals.is_empty() {
        return false;
    }
    
    let mut escapes = HashSet::new();
    
    for block in &body.basic_blocks {
        for stmt in &block.statements {
            match stmt {
                Statement::Assign(place, rvalue) => {
                    // Check if it's assigned to the return place (_0) or to a field of something
                    if let Rvalue::Use(Operand::Copy(p)) | Rvalue::Use(Operand::Move(p)) = rvalue {
                        if !place.projection.is_empty() || place.base == crate::PlaceBase::Local(crate::Local(0)) {
                            escapes.insert(p.base.clone());
                        } else {
                            // If it's assigned to another local, we'll conservatively say it escapes
                            escapes.insert(p.base.clone());
                        }
                    }
                }
                _ => {}
            }
        }
    }
    
    for (base, (bb_idx, stmt_idx)) in class_locals {
        if !escapes.contains(&base) {
            if let Statement::Assign(_, Rvalue::Aggregate(kind, _)) = &mut body.basic_blocks[bb_idx].statements[stmt_idx] {
                if let AggregateKind::Class(name, size) = kind {
                    *kind = AggregateKind::StackClass(*name, *size);
                    changed = true;
                }
            }
        }
    }
    
    changed
}
