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
}
