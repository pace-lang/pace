use super::super::*;

use session::types::{Type, TypeId};

impl<'a> TypeChecker<'a> {
    pub(crate) fn check_match_expr(
        &mut self,
        value: &Expr<'a>,
        arms: &[ast::MatchArm<'a>],
        span: Span,
    ) -> (TypedExprKind<'a>, TypeId) {
        let typed_value = self.check_expr(value);
        let mut typed_arms = Vec::new();
        let mut common_return_type = None;

        for arm in arms {
            self.env.push_scope();

            // Declare bindings in scope
            match &arm.pattern {
                ast::Pattern::Wildcard => {}
                ast::Pattern::Variant { path, bindings } => {
                    if let Some(binds) = bindings {
                        // Extract actual types from the variant
                        let mut extracted_types = Vec::new();

                        let mut enum_name_opt = None;
                        let mut type_args = Vec::new();
                        match self.get_type(typed_value.ty) {
                            Type::GenericInstance(name, args) => {
                                enum_name_opt = Some(name);
                                type_args = args.clone();
                            }
                            Type::Instance(name) => {
                                enum_name_opt = Some(name);
                            }
                            _ => {}
                        }

                        if let Some(enum_name) = enum_name_opt
                            && let Some(variants) = self.enums.get(&enum_name)
                        {
                            let variant_name = path
                                .last()
                                .copied()
                                .unwrap_or_else(|| self.session.interner.borrow_mut().intern(""));
                            if let Some(Type::EnumVariantConstructor(
                                _,
                                _,
                                func_type_params,
                                param_types,
                                _,
                            )) = variants.get(&variant_name).map(|v_ty| self.get_type(*v_ty))
                            {
                                // Substitute generics
                                let mut replacements = std::collections::HashMap::new();
                                for (tp, actual) in func_type_params.iter().zip(type_args.iter()) {
                                    replacements.insert(*tp, *actual);
                                }
                                for pt in param_types {
                                    extracted_types
                                        .push(self.substitute_generics(pt, &replacements));
                                }
                            }
                        }

                        for (i, bind) in binds.iter().enumerate() {
                            if self.session.interner.borrow().lookup(*bind) != "_" {
                                let bind_ty = extracted_types
                                    .get(i)
                                    .cloned()
                                    .unwrap_or(self.session.types.borrow_mut().intern(Type::Any));
                                self.env.declare_var(*bind, bind_ty, false);
                            }
                        }
                    }
                }
            }

            let typed_body = self.check_expr(arm.body);
            self.env.pop_scope();

            if let Some(ref crt) = common_return_type {
                if !self.is_assignable(typed_body.ty, *crt)
                    && typed_body.ty != self.session.types.borrow_mut().intern(Type::Error)
                    && *crt != self.session.types.borrow_mut().intern(Type::Error)
                {
                    if self.is_assignable(*crt, typed_body.ty) {
                        // Promote crt
                        common_return_type = Some(typed_body.ty);
                    } else {
                        self.error(span, DiagnosticCode::TypeMismatch, &format!("Match arms have incompatible return types. Expected '{}', found '{}'.", self.session.format_type(*crt), self.session.format_type(typed_body.ty)));
                    }
                }
            } else {
                common_return_type = Some(typed_body.ty);
            }

            typed_arms.push(ast::TypedMatchArm {
                pattern: arm.pattern.clone(),
                body: self.alloc(typed_body),
            });
        }

        // Exhaustiveness checking should happen here.

        let ty = common_return_type.unwrap_or(self.session.types.borrow_mut().intern(Type::Void));
        (
            TypedExprKind::Match {
                value: self.alloc(typed_value),
                arms: typed_arms,
            },
            ty,
        )
    }
}
