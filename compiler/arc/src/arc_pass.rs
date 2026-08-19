use mir::{Inst, Place, Program, RValue, Terminator, Value};
use std::collections::HashSet;

pub struct ArcPass;

impl Default for ArcPass {
    fn default() -> Self {
        Self::new()
    }
}

impl ArcPass {
    pub fn new() -> Self {
        Self
    }

    pub fn run(&self, program: &mut Program) {
        let mut function_returns_reference = std::collections::HashMap::new();
        for (name, func) in &program.functions {
            function_returns_reference.insert(name.clone(), func.returns_reference);
        }

        for func in program.functions.values_mut() {
            let mut reference_places = HashSet::new();
            let mut owned_places = HashSet::new();

            let is_weak = |place: &Place| -> bool {
                match place {
                    Place::Var(n) => func.weak_vars.contains(n),
                    _ => false,
                }
            };

            for param in &func.reference_parameters {
                reference_places.insert(Place::Var(param.clone()));
            }

            let mut max_temp = 0;
            for block in &func.blocks {
                for inst in &block.instructions {
                    if let Inst::Assign(Place::Temp(id), _) = inst
                        && *id > max_temp
                    {
                        max_temp = *id;
                    }
                }
            }
            let mut temp_counter = max_temp + 1;

            for block in &mut func.blocks {
                let mut new_instructions = Vec::new();
                for inst in &block.instructions {
                    match inst {
                        Inst::Assign(place, RValue::Call(func_name, args)) => {
                            new_instructions.push(Inst::Assign(
                                place.clone(),
                                RValue::Call(func_name.clone(), args.clone()),
                            ));
                            if function_returns_reference.get(func_name) == Some(&true) {
                                reference_places.insert(place.clone());
                                owned_places.insert(place.clone());
                            }
                        }
                        Inst::Assign(place, RValue::AllocateObject(_)) => {
                            new_instructions.push(inst.clone());
                            reference_places.insert(place.clone());
                            owned_places.insert(place.clone());
                        }
                        Inst::Assign(place, RValue::ConstructVariant(_, _, payloads)) => {
                            for payload in payloads {
                                if let Value::Place(p) = payload
                                    && reference_places.contains(p)
                                {
                                    new_instructions.push(Inst::Retain(payload.clone()));
                                }
                            }
                            new_instructions.push(inst.clone());
                            reference_places.insert(place.clone());
                            owned_places.insert(place.clone());
                        }
                        Inst::Assign(place, RValue::ExtractPayload(_val, _, _, is_ref)) => {
                            new_instructions.push(inst.clone());
                            if *is_ref {
                                new_instructions.push(Inst::Retain(Value::Place(place.clone())));
                                reference_places.insert(place.clone());
                                owned_places.insert(place.clone());
                            }
                        }
                        Inst::Assign(place, RValue::Array(vals, is_ref)) => {
                            let mut is_ref_mut = *is_ref;
                            if vals.iter().any(|v| {
                                if let Value::Place(p) = v {
                                    reference_places.contains(p)
                                } else {
                                    false
                                }
                            }) {
                                is_ref_mut = true;
                            }
                            new_instructions.push(Inst::Assign(
                                place.clone(),
                                RValue::Array(vals.clone(), is_ref_mut),
                            ));
                            reference_places.insert(place.clone());
                            owned_places.insert(place.clone());
                        }
                        Inst::Assign(place, RValue::ArrayRepeat(val, count, is_ref)) => {
                            let mut is_ref_mut = *is_ref;
                            if let Value::Place(p) = val
                                && reference_places.contains(p)
                            {
                                is_ref_mut = true;
                            }
                            new_instructions.push(Inst::Assign(
                                place.clone(),
                                RValue::ArrayRepeat(val.clone(), count.clone(), is_ref_mut),
                            ));
                            reference_places.insert(place.clone());
                            owned_places.insert(place.clone());
                        }
                        Inst::Assign(place, RValue::Use(Value::Place(src_place))) => {
                            if reference_places.contains(src_place) || is_weak(src_place) {
                                reference_places.insert(place.clone());
                                owned_places.insert(place.clone());

                                if is_weak(place) {
                                    new_instructions.push(inst.clone());
                                    new_instructions
                                        .push(Inst::WeakRetain(Value::Place(src_place.clone())));
                                } else if is_weak(src_place) {
                                    new_instructions.push(Inst::Assign(
                                        place.clone(),
                                        RValue::WeakUpgrade(Value::Place(src_place.clone())),
                                    ));
                                } else {
                                    new_instructions.push(inst.clone());
                                    new_instructions
                                        .push(Inst::Retain(Value::Place(src_place.clone())));
                                }
                            } else {
                                new_instructions.push(inst.clone());
                            }
                        }
                        Inst::SetProperty(obj_val, prop_name, class_name, val_val, is_ref) => {
                            if *is_ref {
                                let temp_place = Place::Temp(temp_counter);
                                temp_counter += 1;
                                new_instructions.push(Inst::Assign(
                                    temp_place.clone(),
                                    RValue::GetProperty(obj_val.clone(), prop_name.clone(), class_name.clone()),
                                ));
                                new_instructions.push(Inst::Release(Value::Place(temp_place)));
                                new_instructions.push(Inst::Retain(val_val.clone()));
                            }
                            new_instructions.push(inst.clone());
                        }
                        _ => {
                            new_instructions.push(inst.clone());
                        }
                    }
                }

                block.instructions = new_instructions;

                if let Some(Terminator::Return(ret_val)) = &block.terminator {
                    if let Some(Value::Place(ret_place)) = ret_val
                        && reference_places.contains(ret_place)
                        && !owned_places.contains(ret_place)
                    {
                        // Returning a borrowed reference! We must retain it so the caller gets a +1 object!
                        block
                            .instructions
                            .push(Inst::Retain(Value::Place(ret_place.clone())));
                    }

                    let mut sorted_owned_places: Vec<_> = owned_places.iter().collect();
                    sorted_owned_places.sort();
                    for place in sorted_owned_places {
                        let is_returned = match ret_val {
                            Some(Value::Place(p)) => p == place,
                            _ => false,
                        };

                        if !is_returned {
                            if is_weak(place) {
                                block
                                    .instructions
                                    .push(Inst::WeakRelease(Value::Place(place.clone())));
                            } else {
                                block
                                    .instructions
                                    .push(Inst::Release(Value::Place(place.clone())));
                            }
                        }
                    }

                    let mut sorted_struct_places: Vec<_> = func.struct_places.iter().collect();
                    sorted_struct_places.sort_by_key(|(k, _)| (*k).clone());
                    for (place, struct_name) in sorted_struct_places {
                        let is_returned = match ret_val {
                            Some(Value::Place(p)) => p == place,
                            _ => false,
                        };
                        if !is_returned {
                            block
                                .instructions
                                .push(Inst::DropStruct(Value::Place(place.clone()), struct_name.clone()));
                        }
                    }
                }
            }
        }
    }
}
