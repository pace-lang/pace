use super::super::*;
use ast::{TypedExpr, TypedExprKind};
use mir::{Value, Place, RValue, Inst, Terminator};

impl<'a> MirBuilder<'a> {
    pub(crate) fn lower_array_expr(&mut self, elements: &[TypedExpr]) -> Value {
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

    pub(crate) fn lower_array_repeat_expr(&mut self, value: &TypedExpr, count: &TypedExpr) -> Value {
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

    pub(crate) fn lower_list_comprehension_expr(
        &mut self,
        mapped_expr: &TypedExpr,
        item_name: session::Symbol,
        iterator: &TypedExpr,
    ) -> Value {
        match &iterator.kind {
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
                                .lookup(item_name)
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
                                .lookup(item_name)
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
        }
    }
}
