use super::super::*;
use ast::{TypedExpr, TypedMatchArm};
use mir::{Inst, Place, RValue, Terminator, Value};

impl<'a> MirBuilder<'a> {
    pub(crate) fn lower_match_expr(&mut self, value: &TypedExpr, arms: &[TypedMatchArm]) -> Value {
        let match_val = self.lower_expr(value);
        let tag_temp = self.new_temp();
        {
            let __inst = Inst::Assign(tag_temp.clone(), RValue::GetVariantTag(match_val.clone()));
            self.current().instructions.push(__inst)
        };

        let current_block = self.current_block;
        let end_block = self.new_block();
        let result_temp = self.new_temp();

        let mut cases = Vec::new();
        let mut default_block = None;

        let mut enum_name_opt = None;
        let mut type_args = Vec::new();
        match self.session.types.borrow().get(value.ty) {
            session::types::Type::GenericInstance(name, args) => {
                enum_name_opt = Some(*name);
                type_args = args.clone();
            }
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
                        variants
                            .iter()
                            .position(|v| v == self.session.interner.borrow().lookup(*variant_name))
                    })
                    .unwrap_or(0); // Fallback if enum not found
                cases.push((variant_idx, arm_block));

                self.current_block = arm_block;
                if let Some(binds) = bindings {
                    for (field_idx, bind) in binds.iter().enumerate() {
                        if self.session.interner.borrow().lookup(*bind) != "_" {
                            let field_temp = self.new_temp();
                            let mut payload_is_ref = false;
                            {
                                let ty_arena = self.session.types.borrow();
                                for ty in ty_arena.iter() {
                                    if let session::types::Type::EnumVariantConstructor(e_name, v_name, func_type_params, param_types, _) = ty {
                                        if *e_name == resolved_enum_name && *v_name == *variant_name {
                                            if field_idx < param_types.len() {
                                                let mut payload_ty = param_types[field_idx];
                                                if let session::types::Type::Generic(sym) = ty_arena.get(payload_ty) {
                                                    if let Some(idx) = func_type_params.iter().position(|p| p == sym) {
                                                        if idx < type_args.len() {
                                                            payload_ty = type_args[idx];
                                                        }
                                                    }
                                                }
                                                payload_is_ref = super::super::is_ref_type_id(payload_ty, self.session);
                                            }
                                            break;
                                        }
                                    }
                                }
                            }
                            {
                                let __inst = Inst::Assign(
                                    field_temp.clone(),
                                    RValue::ExtractPayload(
                                        match_val.clone(),
                                        variant_idx,
                                        field_idx,
                                        payload_is_ref,
                                    ),
                                );
                                self.current().instructions.push(__inst)
                            };
                            {
                                let __inst = Inst::Assign(
                                    Place::Var(
                                        self.session.interner.borrow().lookup(*bind).to_string(),
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

    pub(crate) fn lower_ternary_expr(
        &mut self,
        condition: &TypedExpr,
        true_expr: &TypedExpr,
        false_expr: &TypedExpr,
    ) -> Value {
        let cond_val = self.lower_expr(condition);

        let then_block = self.new_block();
        let else_block = self.new_block();
        let end_block = self.new_block();
        let result_temp = self.new_temp();

        // Branch
        self.current().terminator = Some(Terminator::Branch {
            cond: cond_val,
            then_block,
            else_block,
        });

        // True branch
        self.current_block = then_block;
        let true_val = self.lower_expr(true_expr);
        {
            let __inst = Inst::Assign(result_temp.clone(), RValue::Use(true_val));
            self.current().instructions.push(__inst)
        };
        self.current().terminator = Some(Terminator::Jump(end_block));

        // False branch
        self.current_block = else_block;
        let false_val = self.lower_expr(false_expr);
        {
            let __inst = Inst::Assign(result_temp.clone(), RValue::Use(false_val));
            self.current().instructions.push(__inst)
        };
        self.current().terminator = Some(Terminator::Jump(end_block));

        self.current_block = end_block;
        Value::Place(result_temp)
    }
}
