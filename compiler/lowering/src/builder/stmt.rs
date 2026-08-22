use super::*;

impl<'a> MirBuilder<'a> {
    pub(crate) fn lower_stmt(&mut self, stmt: &TypedStmt) {
        match &stmt.kind {
            TypedStmtKind::Block(stmts) => {
                for s in stmts {
                    self.lower_stmt(s);
                }
            }
            TypedStmtKind::Binding {
                name,
                initializer,
                is_weak,
                ..
            } => {
                if *is_weak {
                    self.function
                        .weak_vars
                        .insert(self.session.interner.borrow().lookup(*name).to_string());
                }
                if let Some(init) = initializer {
                    let val = self.lower_expr(init);
                    self.emit_assignment(Place::Var(self.session.interner.borrow().lookup(*name).to_string()), init.ty, RValue::Use(val));
                } else {
                    let __inst = Inst::Assign(
                        Place::Var(self.session.interner.borrow().lookup(*name).to_string()),
                        RValue::Use(Value::Void),
                    );
                    self.current().instructions.push(__inst);
                }
            }
            TypedStmtKind::Expression(expr) => {
                self.lower_expr(expr);
            }
            TypedStmtKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond_val = self.lower_expr(condition);

                let then_block = self.new_block();
                let merge_block = self.new_block();

                let else_block = if else_branch.is_some() {
                    self.new_block()
                } else {
                    merge_block
                };

                self.current().terminator = Some(Terminator::Branch {
                    cond: cond_val,
                    then_block,
                    else_block,
                });

                self.current_block = then_block;
                self.lower_stmt(then_branch);
                if self.current().terminator.is_none() {
                    self.current().terminator = Some(Terminator::Jump(merge_block));
                }

                if let Some(e_branch) = else_branch {
                    self.current_block = else_block;
                    self.lower_stmt(e_branch);
                    if self.current().terminator.is_none() {
                        self.current().terminator = Some(Terminator::Jump(merge_block));
                    }
                }

                self.current_block = merge_block;
            }
            TypedStmtKind::While { condition, body } => {
                let cond_block = self.new_block();
                let body_block = self.new_block();
                let merge_block = self.new_block();

                self.current().terminator = Some(Terminator::Jump(cond_block));
                self.current_block = cond_block;

                let cond_val = self.lower_expr(condition);
                self.current().terminator = Some(Terminator::Branch {
                    cond: cond_val,
                    then_block: body_block,
                    else_block: merge_block,
                });

                self.current_block = body_block;
                self.lower_stmt(body);
                if self.current().terminator.is_none() {
                    self.current().terminator = Some(Terminator::Jump(cond_block));
                }

                self.current_block = merge_block;
            }
            TypedStmtKind::Func { .. }
            | TypedStmtKind::Class { .. }
            | TypedStmtKind::Struct { .. }
            | TypedStmtKind::Interface { .. }
            | TypedStmtKind::ForeignFunc { .. }
            | TypedStmtKind::TypeAlias { .. }
            | TypedStmtKind::Extension { .. }
            | TypedStmtKind::Enum { .. } => {
                // Declarations are already processed during `collect_items`
            }
            TypedStmtKind::For {
                item_name,
                iterator,
                body,
                item_ty,
            } => {
                match &iterator.kind {
                    TypedExprKind::Range { start, end } => {
                        let start_val = self.lower_expr(start);
                        let end_val = self.lower_expr(end);

                        let current_var = self.new_temp();
                        {
                            let __inst = Inst::Assign(current_var.clone(), RValue::Use(start_val));
                            self.current().instructions.push(__inst)
                        };

                        let cond_block = self.new_block();
                        let body_block = self.new_block();
                        let merge_block = self.new_block();

                        self.current().terminator = Some(Terminator::Jump(cond_block));

                        // Condition Block
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

                        // Body Block
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
                        self.lower_stmt(body);

                        // Increment
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
                            let __inst = Inst::Assign(
                                current_var.clone(),
                                RValue::Use(Value::Place(inc_temp)),
                            );
                            self.current().instructions.push(__inst)
                        };

                        self.current().terminator = Some(Terminator::Jump(cond_block));

                        // Merge Block
                        self.current_block = merge_block;
                    }
                    _ => {
                        let is_iterable = matches!(
                            self.session.types.borrow().get(iterator.ty),
                            session::types::Type::Instance(_)
                        );

                        if is_iterable {
                            let iter_obj = self.lower_expr(iterator);
                            let iter_call = self.new_temp();
                            let iter_sym = self.session.interner.borrow_mut().intern("iter");

                            {
                                let __inst = Inst::Assign(
                                    iter_call.clone(),
                                    RValue::MethodCall(
                                        iter_obj,
                                        self.session.interner.borrow().lookup(iter_sym).to_string(),
                                        vec![],
                                    ),
                                );
                                self.current().instructions.push(__inst);
                            }

                            let cond_block = self.new_block();
                            let body_block = self.new_block();
                            let merge_block = self.new_block();

                            self.current().terminator = Some(Terminator::Jump(cond_block));
                            self.current_block = cond_block;

                            let next_call = self.new_temp();
                            let next_sym = self.session.interner.borrow_mut().intern("next");

                            {
                                let __inst = Inst::Assign(
                                    next_call.clone(),
                                    RValue::MethodCall(
                                        Value::Place(iter_call.clone()),
                                        self.session.interner.borrow().lookup(next_sym).to_string(),
                                        vec![],
                                    ),
                                );
                                self.current().instructions.push(__inst);
                            }

                            let tag_temp = self.new_temp();
                            {
                                let __inst = Inst::Assign(
                                    tag_temp.clone(),
                                    RValue::GetVariantTag(Value::Place(next_call.clone())),
                                );
                                self.current().instructions.push(__inst);
                            }

                            let cond_temp = self.new_temp();
                            {
                                let __inst = Inst::Assign(
                                    cond_temp.clone(),
                                    RValue::BinaryOp(
                                        ast::BinaryOp::Equal,
                                        Value::Place(tag_temp),
                                        Value::Int(0),
                                    ),
                                );
                                self.current().instructions.push(__inst);
                            }

                            self.current().terminator = Some(Terminator::Branch {
                                cond: Value::Place(cond_temp),
                                then_block: body_block,
                                else_block: merge_block,
                            });

                            self.current_block = body_block;

                            let item_temp = self.new_temp();
                            {
                                let __inst = Inst::Assign(
                                    item_temp.clone(),
                                    RValue::ExtractPayload(
                                        {
                                            let target_name_raw = self.session.format_type(*item_ty);
                                            let type_name = target_name_raw
                                                .replace("<", "_")
                                                .replace(">", "")
                                                .replace(" ", "")
                                                .replace(",", "_");
                                            format!("Option_{}", type_name)
                                        },
                                        Value::Place(next_call.clone()),
                                        0,
                                        0,
                                        crate::builder::is_ref_type_id(*item_ty, self.session, &self.struct_names),
                                    ),
                                );
                                self.current().instructions.push(__inst);
                            }

                            {
                                let __inst = Inst::Assign(
                                    Place::Var(
                                        self.session
                                            .interner
                                            .borrow()
                                            .lookup(*item_name)
                                            .to_string(),
                                    ),
                                    RValue::Use(Value::Place(item_temp)),
                                );
                                self.current().instructions.push(__inst);
                            }

                            self.lower_stmt(body);

                            self.current().terminator = Some(Terminator::Jump(cond_block));
                            self.current_block = merge_block;
                        } else {
                            // Array Iteration
                            let arr_val = self.lower_expr(iterator);

                            let len_var = self.new_temp();
                            {
                                let __inst = Inst::Assign(
                                    len_var.clone(),
                                    RValue::ArrayLength(arr_val.clone()),
                                );
                                self.current().instructions.push(__inst)
                            };

                            let idx_var = self.new_temp();
                            {
                                let __inst =
                                    Inst::Assign(idx_var.clone(), RValue::Use(Value::Int(0)));
                                self.current().instructions.push(__inst)
                            };

                            let cond_block = self.new_block();
                            let body_block = self.new_block();
                            let merge_block = self.new_block();

                            self.current().terminator = Some(Terminator::Jump(cond_block));

                            // Condition Block
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

                            // Body Block
                            self.current_block = body_block;
                            let item_var = self.new_temp();
                            {
                                let __inst = Inst::Assign(
                                    item_var.clone(),
                                    RValue::IndexGet(
                                        arr_val.clone(),
                                        Value::Place(idx_var.clone()),
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
                                            .lookup(*item_name)
                                            .to_string(),
                                    ),
                                    RValue::Use(Value::Place(item_var.clone())),
                                );
                                self.current().instructions.push(__inst)
                            };
                            self.lower_stmt(body);

                            // Increment
                            let inc_temp = self.new_temp();
                            {
                                let __inst = Inst::Assign(
                                    inc_temp.clone(),
                                    RValue::BinaryOp(
                                        ast::BinaryOp::Add,
                                        Value::Place(idx_var.clone()),
                                        Value::Int(1),
                                    ),
                                );
                                self.current().instructions.push(__inst)
                            };
                            {
                                let __inst = Inst::Assign(
                                    idx_var.clone(),
                                    RValue::Use(Value::Place(inc_temp)),
                                );
                                self.current().instructions.push(__inst)
                            };

                            self.current().terminator = Some(Terminator::Jump(cond_block));

                            // Merge Block
                            self.current_block = merge_block;
                        }
                    }
                }
            }
            TypedStmtKind::Return { value } => {
                let val = value.as_ref().map(|v| self.lower_expr(v));
                self.current().terminator = Some(Terminator::Return(val));
                self.current_block = self.new_block(); // Any following code goes to a dead block
            }
        }
    }
}
