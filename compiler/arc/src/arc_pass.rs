use mir::{Inst, Place, Program, RValue, Terminator, Value};
use std::collections::HashSet;

pub struct ArcPass;

impl ArcPass {
    pub fn new() -> Self {
        Self
    }

    pub fn run(&self, program: &mut Program) {
        for (_, func) in &mut program.functions {
            // Very naive ARC pass for Phase 1
            // 1. Identify which places hold objects (AllocateObject)
            // 2. Insert Release(place) at the end of blocks that return, for all object places created in that block.
            
            let mut object_places = HashSet::new();

            for block in &mut func.blocks {
                let mut new_instructions = Vec::new();
                for inst in &block.instructions {
                    match inst {
                        Inst::Assign(place, RValue::Call(func_name, _args)) => {
                            if program.classes.contains_key(func_name) {
                                // Rewrite to AllocateObject
                                new_instructions.push(Inst::Assign(place.clone(), RValue::AllocateObject(func_name.clone())));
                                object_places.insert(place.clone());
                            } else {
                                new_instructions.push(inst.clone());
                            }
                        }
                        Inst::Assign(place, RValue::AllocateObject(_)) => {
                            new_instructions.push(inst.clone());
                            object_places.insert(place.clone());
                        }
                        Inst::Assign(place, RValue::Use(Value::Place(src_place))) => {
                            new_instructions.push(inst.clone());
                            if object_places.contains(src_place) {
                                object_places.insert(place.clone());
                                // Retain the new binding
                                new_instructions.push(Inst::Retain(Value::Place(src_place.clone())));
                            }
                        }
                        Inst::SetProperty(_, _, _) => {
                            new_instructions.push(inst.clone());
                            // If we assign an object to a property, we should retain it.
                            // Skipping for M5 basic test.
                        }
                        _ => {
                            new_instructions.push(inst.clone());
                        }
                    }
                }
                
                block.instructions = new_instructions;
                
                // Release all local objects at the end of the block (very naive, assumes single block or simple control flow for M5)
                if let Some(Terminator::Return(_)) = &block.terminator {
                    for place in &object_places {
                        block.instructions.push(Inst::Release(Value::Place(place.clone())));
                    }
                }
            }
        }
    }
}
