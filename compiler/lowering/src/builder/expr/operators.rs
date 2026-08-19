use super::super::*;
use ast::{BinaryOp, TypedExpr, TypedExprKind, UnaryOp};
use mir::{Inst, Place, RValue, Terminator, Value};

impl<'a> MirBuilder<'a> {
    pub(crate) fn lower_unary_expr(&mut self, op: &UnaryOp, right: &TypedExpr) -> Value {
        let right_val = self.lower_expr(right);
        let temp = self.new_temp();
        {
            let __inst = Inst::Assign(temp.clone(), RValue::UnaryOp(op.clone(), right_val));
            self.current().instructions.push(__inst)
        };
        Value::Place(temp)
    }

    pub(crate) fn lower_binary_expr(
        &mut self,
        left: &TypedExpr,
        op: &BinaryOp,
        right: &TypedExpr,
    ) -> Value {
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

    pub(crate) fn lower_range_expr(&mut self) -> Value {
        unreachable!("Range expressions should only be evaluated as iterators in for loops")
    }

    pub(crate) fn lower_null_coalesce_expr(
        &mut self,
        left: &TypedExpr,
        right: &TypedExpr,
    ) -> Value {
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

    pub(crate) fn lower_null_coalesce_assign_expr(
        &mut self,
        left: &TypedExpr,
        right: &TypedExpr,
    ) -> Value {
        match &left.kind {
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
                        RValue::BinaryOp(ast::BinaryOp::Equal, current_val.clone(), Value::Null),
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
                
                let class_name = match self.session.types.borrow().get(object.ty).clone() {
                    session::types::Type::Class(class_name, _) => {
                        self.session.interner.borrow().lookup(class_name).to_string()
                    }
                    session::types::Type::Struct(struct_name, _) => {
                        self.session.interner.borrow().lookup(struct_name).to_string()
                    }
                    session::types::Type::Instance(name) => {
                        self.session.interner.borrow().lookup(name).to_string()
                    }
                    session::types::Type::Pointer(inner) => {
                        match self.session.types.borrow().get(inner).clone() {
                            session::types::Type::Instance(name) | session::types::Type::Class(name, _) | session::types::Type::Struct(name, _) => {
                                self.session.interner.borrow().lookup(name).to_string()
                            }
                            t => panic!("GetProperty on pointer to non-class/struct: {:?}", t),
                        }
                    }
                    t => panic!("GetProperty on non-class/struct: {:?}", t),
                };

                let current_temp = self.new_temp();
                {
                    let __inst = Inst::Assign(
                        current_temp.clone(),
                        RValue::GetProperty(
                            obj_val.clone(),
                            self.session.interner.borrow().lookup(*name).to_string(),
                            class_name.clone(),
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
                        RValue::BinaryOp(ast::BinaryOp::Equal, current_val.clone(), Value::Null),
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
                        class_name,
                        right_val.clone(),
                        super::super::is_ref_type_id(right.ty, self.session, &self.struct_names),
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
                        RValue::BinaryOp(ast::BinaryOp::Equal, current_val.clone(), Value::Null),
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
        }
    }

    pub(crate) fn lower_force_unwrap_expr(&mut self, inner: &TypedExpr) -> Value {
        let inner_val = self.lower_expr(inner);
        let temp = self.new_temp();
        {
            let __inst = Inst::Assign(temp.clone(), RValue::ForceUnwrap(inner_val));
            self.current().instructions.push(__inst)
        };
        Value::Place(temp)
    }

    pub(crate) fn lower_postfix_try_expr(&mut self, inner: &TypedExpr) -> Value {
        let inner_val = self.lower_expr(inner);
        let tag_temp = self.new_temp();
        {
            let __inst = Inst::Assign(tag_temp.clone(), RValue::GetVariantTag(inner_val.clone()));
            self.current().instructions.push(__inst)
        };

        let then_block = self.new_block();
        let else_block = self.new_block();

        self.current().terminator = Some(Terminator::Switch {
            cond: Value::Place(tag_temp),
            cases: vec![(0, then_block), (1, else_block)],
            default: None,
        });

        let (ok_ty, err_ty) = match self.session.types.borrow().get(inner.ty) {
            session::types::Type::GenericInstance(sym, args) if args.len() == 2 => {
                (args[0], args[1])
            }
            _ => (session::TypeId(0), session::TypeId(0)),
        };

        // Err branch
        self.current_block = else_block;
        let err_payload = self.new_temp();
        let is_err_ref = super::super::is_ref_type_id(err_ty, self.session, &self.struct_names);
        self.current().instructions.push(Inst::Assign(
            err_payload.clone(),
            RValue::ExtractPayload(inner_val.clone(), 1, 0, is_err_ref),
        ));

        let err_result = self.new_temp();
        self.current().instructions.push(Inst::Assign(
            err_result.clone(),
            RValue::ConstructVariant("Result".to_string(), 1, vec![Value::Place(err_payload)]),
        ));

        self.current().terminator = Some(Terminator::Return(Some(Value::Place(err_result))));

        // Ok branch
        self.current_block = then_block;
        let ok_payload = self.new_temp();
        let is_ok_ref = super::super::is_ref_type_id(ok_ty, self.session, &self.struct_names);
        self.current().instructions.push(Inst::Assign(
            ok_payload.clone(),
            RValue::ExtractPayload(inner_val, 0, 0, is_ok_ref),
        ));

        Value::Place(ok_payload)
    }

    pub(crate) fn lower_grouping_expr(&mut self, inner: &TypedExpr) -> Value {
        self.lower_expr(inner)
    }
}
