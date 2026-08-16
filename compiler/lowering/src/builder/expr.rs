use super::*;

impl<'a> MirBuilder<'a> {

    pub(crate) fn lower_expr(&mut self, expr: &TypedExpr) -> Value {
        match &expr.kind {
            TypedExprKind::Integer(i) => Value::Int(*i),
            TypedExprKind::Float(f) => Value::Float(*f),
            TypedExprKind::String(s) => {
                Value::String(self.session.interner.borrow().lookup(*s).to_string())
            }
            TypedExprKind::InterpolatedString(pieces) => {
                if pieces.is_empty() {
                    return Value::String("".to_string());
                }

                let mut current_str_val = None;

                for piece in pieces {
                    let mut piece_val = self.lower_expr(piece);

                    match self.session.types.borrow().get(piece.ty) {
                        session::types::Type::Int => {
                            let temp = self.new_temp();
                            {
                                let __inst = Inst::Assign(
                                    temp.clone(),
                                    RValue::Call("pace_int_to_string".to_string(), vec![piece_val]),
                                );
                                self.current().instructions.push(__inst)
                            };
                            piece_val = Value::Place(temp);
                        }
                        session::types::Type::Float => {
                            let temp = self.new_temp();
                            {
                                let __inst = Inst::Assign(
                                    temp.clone(),
                                    RValue::Call(
                                        "pace_float_to_string".to_string(),
                                        vec![piece_val],
                                    ),
                                );
                                self.current().instructions.push(__inst)
                            };
                            piece_val = Value::Place(temp);
                        }
                        session::types::Type::Boolean => {
                            let temp = self.new_temp();
                            {
                                let __inst = Inst::Assign(
                                    temp.clone(),
                                    RValue::Call(
                                        "pace_bool_to_string".to_string(),
                                        vec![piece_val],
                                    ),
                                );
                                self.current().instructions.push(__inst)
                            };
                            piece_val = Value::Place(temp);
                        }
                        _ => {}
                    }

                    if let Some(current) = current_str_val {
                        let temp = self.new_temp();
                        {
                            let __inst = Inst::Assign(
                                temp.clone(),
                                RValue::Call(
                                    "pace_string_concat".to_string(),
                                    vec![current, piece_val],
                                ),
                            );
                            self.current().instructions.push(__inst)
                        };
                        current_str_val = Some(Value::Place(temp));
                    } else {
                        current_str_val = Some(piece_val);
                    }
                }

                current_str_val.unwrap()
            }
            TypedExprKind::Boolean(b) => Value::Boolean(*b),
            TypedExprKind::Null => Value::Null,
            TypedExprKind::Variable(name) => Value::Place(Place::Var(
                self.session.interner.borrow().lookup(*name).to_string(),
            )),
            TypedExprKind::Array(elements) => {
                let mut vals = Vec::new();
                for el in elements {
                    vals.push(self.lower_expr(el));
                }
                let temp = self.new_temp();
                {
                    let __inst = Inst::Assign(temp.clone(), RValue::Array(vals, false));
                    self.current().instructions.push(__inst)
                };
                Value::Place(temp)
            }
            TypedExprKind::ArrayRepeat { value, count } => {
                let val = self.lower_expr(value);
                let count_val = self.lower_expr(count);
                let temp = self.new_temp();
                {
                    let __inst =
                        Inst::Assign(temp.clone(), RValue::ArrayRepeat(val, count_val, false));
                    self.current().instructions.push(__inst)
                };
                Value::Place(temp)
            }
            TypedExprKind::ListComprehension {
                expr: mapped_expr,
                item_name,
                iterator,
            } => match &iterator.kind {
                TypedExprKind::Range { start, end } => {
                    let start_val = self.lower_expr(start);
                    let end_val = self.lower_expr(end);

                    let len_var = self.new_temp();
                    {
                        let __inst = Inst::Assign(
                            len_var.clone(),
                            RValue::BinaryOp(
                                ast::BinaryOp::Subtract,
                                end_val.clone(),
                                start_val.clone(),
                            ),
                        );
                        self.current().instructions.push(__inst)
                    };

                    let arr_var = self.new_temp();
                    {
                        let __inst = Inst::Assign(
                            arr_var.clone(),
                            RValue::ArrayRepeat(
                                Value::Int(0),
                                Value::Place(len_var.clone()),
                                false,
                            ),
                        );
                        self.current().instructions.push(__inst)
                    };

                    let current_var = self.new_temp();
                    {
                        let __inst = Inst::Assign(current_var.clone(), RValue::Use(start_val));
                        self.current().instructions.push(__inst)
                    };

                    let idx_var = self.new_temp();
                    {
                        let __inst = Inst::Assign(idx_var.clone(), RValue::Use(Value::Int(0)));
                        self.current().instructions.push(__inst)
                    };

                    let cond_block = self.new_block();
                    let body_block = self.new_block();
                    let merge_block = self.new_block();

                    self.current().terminator = Some(Terminator::Jump(cond_block));

                    self.current_block = cond_block;
                    let cond_temp = self.new_temp();
                    {
                        let __inst = Inst::Assign(
                            cond_temp.clone(),
                            RValue::BinaryOp(
                                ast::BinaryOp::Less,
                                Value::Place(current_var.clone()),
                                end_val,
                            ),
                        );
                        self.current().instructions.push(__inst)
                    };
                    self.current().terminator = Some(Terminator::Branch {
                        cond: Value::Place(cond_temp),
                        then_block: body_block,
                        else_block: merge_block,
                    });

                    self.current_block = body_block;
                    {
                        let __inst = Inst::Assign(
                            Place::Var(
                                self.session
                                    .interner
                                    .borrow()
                                    .lookup(*item_name)
                                    .to_string(),
                            ),
                            RValue::Use(Value::Place(current_var.clone())),
                        );
                        self.current().instructions.push(__inst)
                    };

                    let mapped_val = self.lower_expr(mapped_expr);
                    {
                        let __inst = Inst::IndexSet(
                            Value::Place(arr_var.clone()),
                            Value::Place(idx_var.clone()),
                            mapped_val,
                        );
                        self.current().instructions.push(__inst)
                    };

                    let inc_temp = self.new_temp();
                    {
                        let __inst = Inst::Assign(
                            inc_temp.clone(),
                            RValue::BinaryOp(
                                ast::BinaryOp::Add,
                                Value::Place(current_var.clone()),
                                Value::Int(1),
                            ),
                        );
                        self.current().instructions.push(__inst)
                    };
                    {
                        let __inst =
                            Inst::Assign(current_var.clone(), RValue::Use(Value::Place(inc_temp)));
                        self.current().instructions.push(__inst)
                    };

                    let inc_idx_temp = self.new_temp();
                    {
                        let __inst = Inst::Assign(
                            inc_idx_temp.clone(),
                            RValue::BinaryOp(
                                ast::BinaryOp::Add,
                                Value::Place(idx_var.clone()),
                                Value::Int(1),
                            ),
                        );
                        self.current().instructions.push(__inst)
                    };
                    {
                        let __inst =
                            Inst::Assign(idx_var.clone(), RValue::Use(Value::Place(inc_idx_temp)));
                        self.current().instructions.push(__inst)
                    };

                    self.current().terminator = Some(Terminator::Jump(cond_block));

                    self.current_block = merge_block;
                    Value::Place(arr_var)
                }
                _ => {
                    let iter_val = self.lower_expr(iterator);

                    let len_var = self.new_temp();
                    {
                        let __inst =
                            Inst::Assign(len_var.clone(), RValue::ArrayLength(iter_val.clone()));
                        self.current().instructions.push(__inst)
                    };

                    let arr_var = self.new_temp();
                    {
                        let __inst = Inst::Assign(
                            arr_var.clone(),
                            RValue::ArrayRepeat(
                                Value::Int(0),
                                Value::Place(len_var.clone()),
                                false,
                            ),
                        );
                        self.current().instructions.push(__inst)
                    };

                    let idx_var = self.new_temp();
                    {
                        let __inst = Inst::Assign(idx_var.clone(), RValue::Use(Value::Int(0)));
                        self.current().instructions.push(__inst)
                    };

                    let cond_block = self.new_block();
                    let body_block = self.new_block();
                    let merge_block = self.new_block();

                    self.current().terminator = Some(Terminator::Jump(cond_block));

                    self.current_block = cond_block;
                    let cond_temp = self.new_temp();
                    {
                        let __inst = Inst::Assign(
                            cond_temp.clone(),
                            RValue::BinaryOp(
                                ast::BinaryOp::Less,
                                Value::Place(idx_var.clone()),
                                Value::Place(len_var.clone()),
                            ),
                        );
                        self.current().instructions.push(__inst)
                    };
                    self.current().terminator = Some(Terminator::Branch {
                        cond: Value::Place(cond_temp),
                        then_block: body_block,
                        else_block: merge_block,
                    });

                    self.current_block = body_block;
                    let item_var = self.new_temp();
                    {
                        let __inst = Inst::Assign(
                            item_var.clone(),
                            RValue::IndexGet(iter_val.clone(), Value::Place(idx_var.clone())),
                        );
                        self.current().instructions.push(__inst)
                    };
                    {
                        let __inst = Inst::Assign(
                            Place::Var(
                                self.session
                                    .interner
                                    .borrow()
                                    .lookup(*item_name)
                                    .to_string(),
                            ),
                            RValue::Use(Value::Place(item_var)),
                        );
                        self.current().instructions.push(__inst)
                    };

                    let mapped_val = self.lower_expr(mapped_expr);
                    {
                        let __inst = Inst::IndexSet(
                            Value::Place(arr_var.clone()),
                            Value::Place(idx_var.clone()),
                            mapped_val,
                        );
                        self.current().instructions.push(__inst)
                    };

                    let inc_idx_temp = self.new_temp();
                    {
                        let __inst = Inst::Assign(
                            inc_idx_temp.clone(),
                            RValue::BinaryOp(
                                ast::BinaryOp::Add,
                                Value::Place(idx_var.clone()),
                                Value::Int(1),
                            ),
                        );
                        self.current().instructions.push(__inst)
                    };
                    {
                        let __inst =
                            Inst::Assign(idx_var.clone(), RValue::Use(Value::Place(inc_idx_temp)));
                        self.current().instructions.push(__inst)
                    };

                    self.current().terminator = Some(Terminator::Jump(cond_block));

                    self.current_block = merge_block;
                    Value::Place(arr_var)
                }
            },
            TypedExprKind::Match { value, arms } => {
                let match_val = self.lower_expr(value);
                let tag_temp = self.new_temp();
                {
                    let __inst =
                        Inst::Assign(tag_temp.clone(), RValue::GetVariantTag(match_val.clone()));
                    self.current().instructions.push(__inst)
                };

                let current_block = self.current_block;
                let end_block = self.new_block();
                let result_temp = self.new_temp();

                let mut cases = Vec::new();
                let mut default_block = None;

                let mut enum_name_opt = None;
                match self.session.types.borrow().get(value.ty) {
                    session::types::Type::GenericInstance(name, _) => enum_name_opt = Some(*name),
                    session::types::Type::Instance(name) => enum_name_opt = Some(*name),
                    _ => {}
                }
                let resolved_enum_name =
                    enum_name_opt.unwrap_or(self.session.interner.borrow_mut().intern(""));

                for arm in arms {
                    let arm_block = self.new_block();

                    if let ast::Pattern::Variant { path, bindings } = &arm.pattern {
                        let enum_name = &resolved_enum_name;
                        let variant_name = path.last().unwrap();
                        let variant_idx = self
                            .enums_map
                            .get(self.session.interner.borrow().lookup(*enum_name))
                            .and_then(|variants| {
                                variants.iter().position(|v| {
                                    v == self.session.interner.borrow().lookup(*variant_name)
                                })
                            })
                            .unwrap_or(0); // Fallback if enum not found
                        cases.push((variant_idx, arm_block));

                        self.current_block = arm_block;
                        if let Some(binds) = bindings {
                            for (field_idx, bind) in binds.iter().enumerate() {
                                if self.session.interner.borrow().lookup(*bind) != "_" {
                                    let field_temp = self.new_temp();
                                    {
                                        let __inst = Inst::Assign(
                                            field_temp.clone(),
                                            RValue::ExtractPayload(
                                                match_val.clone(),
                                                variant_idx,
                                                field_idx,
                                            ),
                                        );
                                        self.current().instructions.push(__inst)
                                    };
                                    {
                                        let __inst = Inst::Assign(
                                            Place::Var(
                                                self.session
                                                    .interner
                                                    .borrow()
                                                    .lookup(*bind)
                                                    .to_string(),
                                            ),
                                            RValue::Use(Value::Place(field_temp)),
                                        );
                                        self.current().instructions.push(__inst)
                                    };
                                }
                            }
                        }
                    } else if let ast::Pattern::Wildcard = &arm.pattern {
                        default_block = Some(arm_block);
                        self.current_block = arm_block;
                    }

                    let arm_val = self.lower_expr(arm.body);
                    {
                        let __inst = Inst::Assign(result_temp.clone(), RValue::Use(arm_val));
                        self.current().instructions.push(__inst)
                    };
                    self.current().terminator = Some(Terminator::Jump(end_block));
                }

                let switch_block = &mut self.function.blocks[current_block.0];
                switch_block.terminator = Some(Terminator::Switch {
                    cond: Value::Place(tag_temp),
                    cases,
                    default: default_block,
                });

                self.current_block = end_block;
                Value::Place(result_temp)
            }
            TypedExprKind::EnumVariant {
                enum_name,
                variant_name,
            } => {
                let variant_idx = self
                    .enums_map
                    .get(self.session.interner.borrow().lookup(*enum_name))
                    .and_then(|variants| {
                        variants
                            .iter()
                            .position(|v| v == self.session.interner.borrow().lookup(*variant_name))
                    })
                    .unwrap_or(0);
                let temp = self.new_temp();
                {
                    let __inst = Inst::Assign(
                        temp.clone(),
                        RValue::ConstructVariant(
                            self.session
                                .interner
                                .borrow()
                                .lookup(*enum_name)
                                .to_string(),
                            variant_idx,
                            Vec::new(),
                        ),
                    );
                    self.current().instructions.push(__inst)
                };
                Value::Place(temp)
            }
            TypedExprKind::IndexGet { object, index } => {
                let obj_val = self.lower_expr(object);
                let idx_val = self.lower_expr(index);
                let temp = self.new_temp();
                {
                    let __inst = Inst::Assign(temp.clone(), RValue::IndexGet(obj_val, idx_val));
                    self.current().instructions.push(__inst)
                };
                Value::Place(temp)
            }
            TypedExprKind::IndexSet {
                object,
                index,
                value,
            } => {
                let obj_val = self.lower_expr(object);
                let idx_val = self.lower_expr(index);
                let val_val = self.lower_expr(value);
                {
                    let __inst = Inst::IndexSet(obj_val, idx_val, val_val.clone());
                    self.current().instructions.push(__inst)
                };
                val_val
            }
            TypedExprKind::Grouping(inner) => self.lower_expr(inner),
            TypedExprKind::Get { object, name } => {
                let obj_val = self.lower_expr(object);
                let temp = self.new_temp();
                {
                    let __inst = Inst::Assign(
                        temp.clone(),
                        RValue::GetProperty(
                            obj_val,
                            self.session.interner.borrow().lookup(*name).to_string(),
                        ),
                    );
                    self.current().instructions.push(__inst)
                };
                Value::Place(temp)
            }
            TypedExprKind::PostfixTry(inner) => {
                let inner_val = self.lower_expr(inner);
                let tag_temp = self.new_temp();
                {
                    let __inst =
                        Inst::Assign(tag_temp.clone(), RValue::GetVariantTag(inner_val.clone()));
                    self.current().instructions.push(__inst)
                };

                let then_block = self.new_block();
                let else_block = self.new_block();

                self.current().terminator = Some(Terminator::Switch {
                    cond: Value::Place(tag_temp),
                    cases: vec![(0, then_block), (1, else_block)],
                    default: None,
                });

                // Err branch
                self.current_block = else_block;
                let err_payload = self.new_temp();
                self.current().instructions.push(Inst::Assign(
                    err_payload.clone(),
                    RValue::ExtractPayload(inner_val.clone(), 1, 0),
                ));

                let err_result = self.new_temp();
                self.current().instructions.push(Inst::Assign(
                    err_result.clone(),
                    RValue::ConstructVariant(
                        "Result".to_string(),
                        1,
                        vec![Value::Place(err_payload)],
                    ),
                ));

                self.current().terminator =
                    Some(Terminator::Return(Some(Value::Place(err_result))));

                // Ok branch
                self.current_block = then_block;
                let ok_payload = self.new_temp();
                self.current().instructions.push(Inst::Assign(
                    ok_payload.clone(),
                    RValue::ExtractPayload(inner_val, 0, 0),
                ));

                Value::Place(ok_payload)
            }
            TypedExprKind::ForceUnwrap(inner) => {
                let inner_val = self.lower_expr(inner);
                let temp = self.new_temp();
                {
                    let __inst = Inst::Assign(temp.clone(), RValue::ForceUnwrap(inner_val));
                    self.current().instructions.push(__inst)
                };
                Value::Place(temp)
            }
            TypedExprKind::OptionalGet { object, name } => {
                let obj_val = self.lower_expr(object);
                let temp = self.new_temp();

                let then_block = self.new_block();
                let else_block = self.new_block();
                let merge_block = self.new_block();

                let is_null_temp = self.new_temp();
                {
                    let __inst = Inst::Assign(
                        is_null_temp.clone(),
                        RValue::BinaryOp(ast::BinaryOp::Equal, obj_val.clone(), Value::Null),
                    );
                    self.current().instructions.push(__inst)
                };

                self.current().terminator = Some(Terminator::Branch {
                    cond: Value::Place(is_null_temp),
                    then_block,
                    else_block,
                });

                self.current_block = then_block;
                {
                    let __inst = Inst::Assign(temp.clone(), RValue::Use(Value::Null));
                    self.current().instructions.push(__inst)
                };
                self.current().terminator = Some(Terminator::Jump(merge_block));

                self.current_block = else_block;
                {
                    let __inst = Inst::Assign(
                        temp.clone(),
                        RValue::GetProperty(
                            obj_val,
                            self.session.interner.borrow().lookup(*name).to_string(),
                        ),
                    );
                    self.current().instructions.push(__inst)
                };
                self.current().terminator = Some(Terminator::Jump(merge_block));

                self.current_block = merge_block;
                Value::Place(temp)
            }
            TypedExprKind::NullCoalesce { left, right } => {
                let left_val = self.lower_expr(left);
                let temp = self.new_temp();

                let then_block = self.new_block();
                let else_block = self.new_block();
                let merge_block = self.new_block();

                let is_null_temp = self.new_temp();
                {
                    let __inst = Inst::Assign(
                        is_null_temp.clone(),
                        RValue::BinaryOp(ast::BinaryOp::Equal, left_val.clone(), Value::Null),
                    );
                    self.current().instructions.push(__inst)
                };

                self.current().terminator = Some(Terminator::Branch {
                    cond: Value::Place(is_null_temp),
                    then_block,
                    else_block,
                });

                self.current_block = then_block;
                let right_val = self.lower_expr(right);
                {
                    let __inst = Inst::Assign(temp.clone(), RValue::Use(right_val));
                    self.current().instructions.push(__inst)
                };
                self.current().terminator = Some(Terminator::Jump(merge_block));

                self.current_block = else_block;
                {
                    let __inst = Inst::Assign(temp.clone(), RValue::Use(left_val));
                    self.current().instructions.push(__inst)
                };
                self.current().terminator = Some(Terminator::Jump(merge_block));

                self.current_block = merge_block;
                Value::Place(temp)
            }
            TypedExprKind::NullCoalesceAssign { left, right } => match &left.kind {
                TypedExprKind::Variable(name) => {
                    let current_val = Value::Place(Place::Var(
                        self.session.interner.borrow().lookup(*name).to_string(),
                    ));
                    let then_block = self.new_block();
                    let merge_block = self.new_block();

                    let is_null_temp = self.new_temp();
                    {
                        let __inst = Inst::Assign(
                            is_null_temp.clone(),
                            RValue::BinaryOp(
                                ast::BinaryOp::Equal,
                                current_val.clone(),
                                Value::Null,
                            ),
                        );
                        self.current().instructions.push(__inst)
                    };

                    self.current().terminator = Some(Terminator::Branch {
                        cond: Value::Place(is_null_temp),
                        then_block,
                        else_block: merge_block,
                    });

                    self.current_block = then_block;
                    let right_val = self.lower_expr(right);
                    {
                        let __inst = Inst::Assign(
                            Place::Var(self.session.interner.borrow().lookup(*name).to_string()),
                            RValue::Use(right_val),
                        );
                        self.current().instructions.push(__inst)
                    };
                    self.current().terminator = Some(Terminator::Jump(merge_block));

                    self.current_block = merge_block;
                    current_val
                }
                TypedExprKind::Get { object, name } => {
                    let obj_val = self.lower_expr(object);
                    let current_temp = self.new_temp();
                    {
                        let __inst = Inst::Assign(
                            current_temp.clone(),
                            RValue::GetProperty(
                                obj_val.clone(),
                                self.session.interner.borrow().lookup(*name).to_string(),
                            ),
                        );
                        self.current().instructions.push(__inst)
                    };
                    let current_val = Value::Place(current_temp.clone());

                    let then_block = self.new_block();
                    let merge_block = self.new_block();

                    let is_null_temp = self.new_temp();
                    {
                        let __inst = Inst::Assign(
                            is_null_temp.clone(),
                            RValue::BinaryOp(
                                ast::BinaryOp::Equal,
                                current_val.clone(),
                                Value::Null,
                            ),
                        );
                        self.current().instructions.push(__inst)
                    };

                    self.current().terminator = Some(Terminator::Branch {
                        cond: Value::Place(is_null_temp),
                        then_block,
                        else_block: merge_block,
                    });

                    self.current_block = then_block;
                    let right_val = self.lower_expr(right);
                    {
                        let __inst = Inst::SetProperty(
                            obj_val,
                            self.session.interner.borrow().lookup(*name).to_string(),
                            right_val.clone(),
                        );
                        self.current().instructions.push(__inst)
                    };
                    {
                        let __inst = Inst::Assign(current_temp.clone(), RValue::Use(right_val));
                        self.current().instructions.push(__inst)
                    };
                    self.current().terminator = Some(Terminator::Jump(merge_block));

                    self.current_block = merge_block;
                    current_val
                }
                TypedExprKind::IndexGet { object, index } => {
                    let obj_val = self.lower_expr(object);
                    let idx_val = self.lower_expr(index);

                    let current_temp = self.new_temp();
                    {
                        let __inst = Inst::Assign(
                            current_temp.clone(),
                            RValue::IndexGet(obj_val.clone(), idx_val.clone()),
                        );
                        self.current().instructions.push(__inst)
                    };
                    let current_val = Value::Place(current_temp.clone());

                    let then_block = self.new_block();
                    let merge_block = self.new_block();

                    let is_null_temp = self.new_temp();
                    {
                        let __inst = Inst::Assign(
                            is_null_temp.clone(),
                            RValue::BinaryOp(
                                ast::BinaryOp::Equal,
                                current_val.clone(),
                                Value::Null,
                            ),
                        );
                        self.current().instructions.push(__inst)
                    };

                    self.current().terminator = Some(Terminator::Branch {
                        cond: Value::Place(is_null_temp),
                        then_block,
                        else_block: merge_block,
                    });

                    self.current_block = then_block;
                    let right_val = self.lower_expr(right);
                    {
                        let __inst = Inst::IndexSet(obj_val, idx_val, right_val.clone());
                        self.current().instructions.push(__inst)
                    };
                    {
                        let __inst = Inst::Assign(current_temp.clone(), RValue::Use(right_val));
                        self.current().instructions.push(__inst)
                    };
                    self.current().terminator = Some(Terminator::Jump(merge_block));

                    self.current_block = merge_block;
                    current_val
                }
                _ => unreachable!("Invalid target for NullCoalesceAssign"),
            },
            TypedExprKind::Set {
                object,
                name,
                value,
            } => {
                let obj_val = self.lower_expr(object);
                let val_val = self.lower_expr(value);
                {
                    let __inst = Inst::SetProperty(
                        obj_val,
                        self.session.interner.borrow().lookup(*name).to_string(),
                        val_val.clone(),
                    );
                    self.current().instructions.push(__inst)
                };
                val_val
            }
            TypedExprKind::Assign { name, value } => {
                let val = self.lower_expr(value);
                {
                    let __inst = Inst::Assign(
                        Place::Var(self.session.interner.borrow().lookup(*name).to_string()),
                        RValue::Use(val.clone()),
                    );
                    self.current().instructions.push(__inst)
                };
                val
            }
            TypedExprKind::SelfRef => Value::Place(Place::Var("self".to_string())),
            TypedExprKind::Call {
                callee,
                type_args: _,
                arguments,
            } => {
                let mut arg_values = Vec::new();
                for arg in arguments {
                    arg_values.push(self.lower_expr(arg));
                }

                if let TypedExprKind::Get { object, name } = &callee.kind {
                    let obj_val = self.lower_expr(object);
                    let temp = self.new_temp();

                    if let Type::Instance(class_name) | Type::GenericInstance(class_name, _) =
                        self.session.types.borrow().get(object.ty)
                    {
                        let actual_name = format!(
                            "{}::{}",
                            self.session.interner.borrow().lookup(*class_name),
                            self.session.interner.borrow().lookup(*name)
                        );
                        arg_values.insert(0, obj_val);
                        {
                            let __inst =
                                Inst::Assign(temp.clone(), RValue::Call(actual_name, arg_values));
                            self.current().instructions.push(__inst)
                        };
                        return Value::Place(temp);
                    }

                    {
                        let __inst = Inst::Assign(
                            temp.clone(),
                            RValue::MethodCall(
                                obj_val.clone(),
                                self.session.interner.borrow().lookup(*name).to_string(),
                                arg_values,
                            ),
                        );
                        self.current().instructions.push(__inst)
                    };
                    return Value::Place(temp);
                }

                if let TypedExprKind::EnumVariant {
                    enum_name,
                    variant_name,
                } = &callee.kind
                {
                    let temp = self.new_temp();
                    let variant_idx = self
                        .enums_map
                        .get(self.session.interner.borrow().lookup(*enum_name))
                        .and_then(|variants| {
                            variants.iter().position(|v| {
                                v == self.session.interner.borrow().lookup(*variant_name)
                            })
                        })
                        .unwrap_or(0);
                    {
                        let __inst = Inst::Assign(
                            temp.clone(),
                            RValue::ConstructVariant(
                                self.session
                                    .interner
                                    .borrow()
                                    .lookup(*enum_name)
                                    .to_string(),
                                variant_idx,
                                arg_values,
                            ),
                        );
                        self.current().instructions.push(__inst)
                    };
                    return Value::Place(temp);
                }

                if let Type::Struct(struct_name, _) =
                    self.session.types.borrow().get(callee.ty).clone()
                {
                    let obj_temp = self.new_temp();
                    {
                        let __inst = Inst::Assign(
                            obj_temp.clone(),
                            RValue::AllocateStruct(
                                self.session
                                    .interner
                                    .borrow()
                                    .lookup(struct_name)
                                    .to_string(),
                            ),
                        );
                        self.current().instructions.push(__inst)
                    };

                    let actual_name = format!(
                        "{}::init",
                        self.session.interner.borrow().lookup(struct_name)
                    );
                    arg_values.insert(0, Value::Place(obj_temp.clone()));
                    let init_temp = self.new_temp();
                    {
                        let __inst = Inst::Assign(init_temp, RValue::Call(actual_name, arg_values));
                        self.current().instructions.push(__inst)
                    };

                    return Value::Place(obj_temp);
                }

                if let Type::Class(class_name, _) =
                    self.session.types.borrow().get(callee.ty).clone()
                {
                    let obj_temp = self.new_temp();
                    {
                        let __inst = Inst::Assign(
                            obj_temp.clone(),
                            RValue::AllocateObject(
                                self.session
                                    .interner
                                    .borrow()
                                    .lookup(class_name)
                                    .to_string(),
                            ),
                        );
                        self.current().instructions.push(__inst)
                    };

                    let actual_name = format!(
                        "{}::init",
                        self.session.interner.borrow().lookup(class_name)
                    );
                    arg_values.insert(0, Value::Place(obj_temp.clone()));
                    let init_temp = self.new_temp();
                    {
                        let __inst = Inst::Assign(init_temp, RValue::Call(actual_name, arg_values));
                        self.current().instructions.push(__inst)
                    };

                    return Value::Place(obj_temp);
                }

                let mut func_name = if let TypedExprKind::Variable(name) = &callee.kind {
                    *name
                } else {
                    panic!("Only direct function calls by name are currently supported.");
                };

                if self.session.interner.borrow().lookup(func_name) == "print"
                    && arguments.len() == 1
                {
                    match self.session.types.borrow().get(arguments[0].ty) {
                        session::types::Type::String => {
                            func_name = self.session.interner.borrow_mut().intern("printStr")
                        }
                        session::types::Type::Float => {
                            func_name = self.session.interner.borrow_mut().intern("printFloat")
                        }
                        session::types::Type::Boolean => {
                            func_name = self.session.interner.borrow_mut().intern("printBool")
                        }
                        session::types::Type::Enum(_, _) | session::types::Type::Instance(_) => {
                            func_name = self.session.interner.borrow_mut().intern("printEnum")
                        }
                        _ => func_name = self.session.interner.borrow_mut().intern("printInt"),
                    }
                }

                let temp = self.new_temp();
                {
                    let __inst = Inst::Assign(
                        temp.clone(),
                        RValue::Call(
                            self.session.interner.borrow().lookup(func_name).to_string(),
                            arg_values,
                        ),
                    );
                    self.current().instructions.push(__inst)
                };
                Value::Place(temp)
            }
            TypedExprKind::Unary(op, right) => {
                let right_val = self.lower_expr(right);
                let temp = self.new_temp();
                {
                    let __inst = Inst::Assign(temp.clone(), RValue::UnaryOp(op.clone(), right_val));
                    self.current().instructions.push(__inst)
                };
                Value::Place(temp)
            }
            TypedExprKind::Binary(left, op, right) => {
                let left_ty = left.ty;
                let right_ty = right.ty;
                let left_val = self.lower_expr(left);
                let right_val = self.lower_expr(right);
                let temp = self.new_temp();

                if op == &ast::BinaryOp::Add
                    && left_ty
                        == self
                            .session
                            .types
                            .borrow_mut()
                            .intern(session::types::Type::String)
                    && right_ty
                        == self
                            .session
                            .types
                            .borrow_mut()
                            .intern(session::types::Type::String)
                {
                    {
                        let __inst = Inst::Assign(
                            temp.clone(),
                            RValue::Call("stringConcat".to_string(), vec![left_val, right_val]),
                        );
                        self.current().instructions.push(__inst)
                    };
                } else {
                    {
                        let __inst = Inst::Assign(
                            temp.clone(),
                            RValue::BinaryOp(op.clone(), left_val, right_val),
                        );
                        self.current().instructions.push(__inst)
                    };
                }
                Value::Place(temp)
            }
            TypedExprKind::Range { .. } => {
                unreachable!("Range expressions should only be evaluated as iterators in for loops")
            }
        }
    }
}
