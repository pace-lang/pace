use mir::{Inst, Place, Program, RValue, Terminator, Value};
use std::collections::HashSet;

pub struct ArcPass;

impl ArcPass {
    pub fn new() -> Self {
        Self
    }

    pub fn run(&self, program: &mut Program) {
        for (_, func) in &mut program.functions {
            let mut object_places = HashSet::new();

            let is_weak = |place: &Place| -> bool {
                match place {
                    Place::Var(n) => func.weak_vars.contains(n),
                    _ => false
                }
            };

            for block in &mut func.blocks {
                let mut new_instructions = Vec::new();
                for inst in &block.instructions {
                    match inst {
                        Inst::Assign(place, RValue::Call(func_name, _args)) => {
                            if program.classes.contains_key(func_name) {
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
                            if object_places.contains(src_place) || is_weak(src_place) {
                                object_places.insert(place.clone());
                                
                                if is_weak(place) {
                                    new_instructions.push(inst.clone());
                                    new_instructions.push(Inst::WeakRetain(Value::Place(src_place.clone())));
                                } else if is_weak(src_place) {
                                    new_instructions.push(Inst::Assign(place.clone(), RValue::WeakUpgrade(Value::Place(src_place.clone()))));
                                } else {
                                    new_instructions.push(inst.clone());
                                    new_instructions.push(Inst::Retain(Value::Place(src_place.clone())));
                                }
                            } else {
                                new_instructions.push(inst.clone());
                            }
                        }
                        Inst::SetProperty(_, _, _) => {
                            new_instructions.push(inst.clone());
                        }
                        _ => {
                            new_instructions.push(inst.clone());
                        }
                    }
                }
                
                block.instructions = new_instructions;
                
                if let Some(Terminator::Return(_)) = &block.terminator {
                    for place in &object_places {
                        if is_weak(place) {
                            block.instructions.push(Inst::WeakRelease(Value::Place(place.clone())));
                        } else {
                            block.instructions.push(Inst::Release(Value::Place(place.clone())));
                        }
                    }
                }
            }
        }
    }
}
