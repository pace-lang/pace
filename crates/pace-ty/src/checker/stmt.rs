use super::TypeChecker;
use super::is_camel_case;
use crate::env::Type;
use pace_hir::Stmt;
use pace_errors::TypeError;

impl<'a> TypeChecker<'a> {
    pub(crate) fn check_stmt(&mut self, stmt_id: pace_hir::StmtId) {
        let stmt = self.arena.get_stmt(stmt_id);
        match stmt {
            Stmt::Module { name, body } => {
                let old = self.current_module;
                self.current_module = *name;
                for s in body {
                    self.check_stmt(*s);
                }
                self.current_module = old;
            }
            Stmt::Expr(expr) => {
                self.check_expr(*expr);
            }
            Stmt::VarDecl {
                name,
                is_mutable,
                type_annotation,
                initializer,
                ..
            } => {
                let span = self.arena.get_stmt_span(stmt_id);
                self.check_stmt_var_decl(
                    *name,
                    *is_mutable,
                    type_annotation.as_ref(),
                    *initializer,
                    span,
                );
            }
            Stmt::Block(stmts) => {
                self.env.push_scope();
                for s in stmts {
                    self.check_stmt(*s);
                }
                self.pop_scope_and_check_unused();
            }
            Stmt::Return(expr_opt) => {
                let ret_ty = if let Some(expr) = expr_opt {
                    self.check_expr(*expr)
                } else {
                    Type::Void
                };

                if let Some(expected) = &self.current_return_type {
                    let mut is_match = false;

                    if self.is_assignable_to(&ret_ty, expected)
                        || expected == &Type::Unknown
                        || ret_ty == Type::Unknown
                        || expected == &Type::Any
                        || ret_ty == Type::Any
                    {
                        is_match = true;
                    } else if let Type::Nullable(inner) = expected
                        && (ret_ty == Type::Null || self.is_assignable_to(&ret_ty, inner))
                    {
                        is_match = true;
                    }

                    if !is_match {
                        {
                            self.errors.push(TypeError::Generic {
                                src: self.get_source(),
                                span: self.current_span,
                                message: format!(
                                    "Type mismatch: expected return type {:?}, found {:?}",
                                    expected, ret_ty
                                ),
                            });
                        }
                    }
                } else {
                    {
                        self.errors.push(TypeError::Generic {
                            src: self.get_source(),
                            span: self.current_span,
                            message: "Return statement outside of function".into(),
                        });
                    }
                }
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond_ty = self.check_expr(*condition);
                if cond_ty != Type::Bool && cond_ty != Type::Unknown {
                    {
                        self.errors.push(TypeError::Generic {
                            src: self.get_source(),
                            span: self.current_span,
                            message: "If condition must be a boolean".into(),
                        });
                        return;
                    };
                }
                self.check_stmt(*then_branch);
                if let Some(else_b) = else_branch {
                    self.check_stmt(*else_b);
                }
            }
            Stmt::While { condition, body } => {
                let cond_ty = self.check_expr(*condition);
                if cond_ty != Type::Bool && cond_ty != Type::Unknown {
                    {
                        self.errors.push(TypeError::Generic {
                            src: self.get_source(),
                            span: self.current_span,
                            message: "While condition must be a boolean".into(),
                        });
                        return;
                    };
                }
                self.check_stmt(*body);
            }
            Stmt::Loop { body } => {
                self.check_stmt(*body);
            }
            Stmt::ForIn {
                item,
                iterable,
                body,
                ..
            } => {
                let iterable_ty = self.check_expr(*iterable);
                let mut item_ty = Type::Unknown;

                if let Type::GenericInstance { base: _, args } = &iterable_ty {
                    if !args.is_empty() {
                        item_ty = args[0].clone();
                    }
                } else if let Type::Class(_name) = &iterable_ty {
                    // Fallback for non-generic classes if needed, though most iterables are generic
                }

                self.env.push_scope();
                self.define_var(*item, item_ty, pace_span::Span::default(), false);
                self.check_stmt(*body);
                self.pop_scope_and_check_unused();
            }
            Stmt::Match { expr, arms } => {
                let expr_ty = self.check_expr(*expr);
                for (pattern, body) in arms {
                    self.env.push_scope();
                    self.check_pattern(pattern, &expr_ty);
                    self.check_stmt(*body);
                    self.pop_scope_and_check_unused();
                }
            }
            Stmt::FuncDecl {
                params,
                body,
                return_type,
                generic_params,
                is_static,

                ..
            } => {
                let span = self.arena.get_stmt_span(stmt_id);
                self.check_stmt_func_decl(
                    params,
                    body,
                    return_type.as_ref(),
                    generic_params.as_deref(),
                    *is_static,
                    span,
                );
            }
            Stmt::ClassDecl {
                name,
                methods,
                implements,
                generic_params,
                ..
            }
            | Stmt::ActorDecl {
                name,
                methods,
                implements,
                generic_params,
                ..
            } => {
                self.check_stmt_class_decl(
                    *name,
                    methods,
                    implements.as_ref(),
                    generic_params.as_deref(),
                );
            }
            Stmt::InterfaceDecl { .. } => {}
            Stmt::StructDecl { .. } => {}
            Stmt::EnumDecl { .. } => {}
            Stmt::Import { .. } | Stmt::Export { .. } => {}
        }
    }

    pub(crate) fn check_stmt_var_decl(
        &mut self,
        name: ustr::Ustr,
        is_mutable: bool,
        type_annotation: Option<&pace_ast::TypeAnnotation>,
        initializer: Option<pace_hir::ExprId>,
        span: pace_span::Span,
    ) {
        self.current_span = span;
        let mut inferred_type = Type::Unknown;

        let mut val_ty_copy = Type::Unknown;
        if let Some(init_expr) = initializer {
            inferred_type = self.check_expr(init_expr);
            val_ty_copy = inferred_type.clone();
        }

        if let Some(annotation) = type_annotation {
            let expected_type = self.resolve_type_name(annotation);
            let mut is_match = false;

            if self.is_assignable_to(&inferred_type, &expected_type) {
                is_match = true;
            } else if let Type::Nullable(inner) = &expected_type
                && (inferred_type == Type::Null || self.is_assignable_to(&inferred_type, inner))
            {
                is_match = true;
            }

            if !is_match {
                self.define_var(name, expected_type.clone(), span, is_mutable);
                self.errors.push(TypeError::Generic {
                    src: self.get_source(),
                    span: self.current_span,
                    message: format!(
                        "Type mismatch: expected {:?}, found {:?}",
                        expected_type, inferred_type
                    ),
                });
                return;
            }
            inferred_type = expected_type.clone();
            
            if let Some(init_expr) = initializer {
                self.coerce_expr_if_needed(&expected_type, &val_ty_copy, init_expr);
            }
        }

        if inferred_type == Type::Unknown {
            self.errors.push(TypeError::Generic {
                src: self.get_source(),
                span: self.current_span,
                message: format!("Cannot infer type for variable '{}'", name),
            });
            return;
        }

        if !is_camel_case(name.as_str()) && !name.as_str().contains("__") {
            self.warnings
                .push(pace_errors::SemanticWarning::NamingConvention {
                    name: name.to_string(),
                    src: self.get_source(),
                    span: span,
                });
        }
        self.define_var(name, inferred_type, span, is_mutable);
    }

    pub(crate) fn check_stmt_func_decl(
        &mut self,
        params: &[pace_hir::Param],
        body: &[pace_hir::StmtId],
        return_type: Option<&pace_ast::TypeAnnotation>,
        generic_params: Option<&[pace_ast::GenericParam]>,
        is_static: bool,
        span: pace_span::Span,
    ) {
        self.current_span = span;
        let prev_return = self.current_return_type.clone();
        let prev_generics = self.generic_params_in_scope.clone();

        if let Some(gps) = generic_params {
            self.generic_params_in_scope.extend(gps.to_vec());
        }

        let ret_ty = if let Some(rt) = return_type {
            self.resolve_type_name(rt)
        } else {
            Type::Void
        };
        self.current_return_type = Some(ret_ty);

        self.env.push_scope();

        // Add `self` if we are inside a class/struct AND the method is not static
        if let Some(class_name) = &self.current_class
            && !is_static
        {
            let self_ty = if self.env.structs.contains_key(class_name) {
                Type::Struct(*class_name)
            } else if self.env.actors.contains_key(class_name) {
                Type::Actor(*class_name)
            } else {
                Type::Class(*class_name)
            };
            self.define_var("self".into(), self_ty, pace_span::Span::default(), false);
        }

        // Add parameters to scope
        for param in params {
            let param_type = self.resolve_type_name(&param.type_annotation);
            self.define_var(param.name, param_type, pace_span::Span::default(), false);
        }

        // Check body
        for s in body {
            self.check_stmt(*s);
        }

        self.pop_scope_and_check_unused();
        self.current_return_type = prev_return;
        self.generic_params_in_scope = prev_generics;
    }

    pub(crate) fn check_stmt_class_decl(
        &mut self,
        name: ustr::Ustr,
        methods: &[pace_hir::StmtId],
        implements: Option<&pace_ast::TypeAnnotation>,
        generic_params: Option<&[pace_ast::GenericParam]>,
    ) {
        let prev_class = self.current_class;
        let prev_generics = self.generic_params_in_scope.clone();

        self.current_class = Some(name);

        if let Some(gps) = generic_params {
            self.generic_params_in_scope.extend(gps.to_vec());
        }

        if let Some(iface_annotation) = implements {
            let iface_name = &iface_annotation.name;
            // Check if class actually implements the interface
            if let Some(iface_sig) = self.env.interfaces.get(iface_name) {
                let class_sig = self.env.classes.get(&name).unwrap().clone();
                for m_name in iface_sig.methods.keys() {
                    if class_sig.methods.get(m_name).is_some() {
                        // For simplicity, we just check if it exists right now
                        // In a full compiler, we'd check parameter counts and types
                    } else {
                        self.errors.push(TypeError::Generic {
                            src: self.get_source(),
                            span: self.current_span,
                            message: format!(
                                "Class '{}' does not implement method '{}' from interface '{}'",
                                name, m_name, iface_name
                            ),
                        });
                        return;
                    }
                }
            } else {
                self.errors.push(TypeError::Generic {
                    src: self.get_source(),
                    span: self.current_span,
                    message: format!("Interface '{}' not found", iface_name),
                });
                return;
            }
        }

        self.env.push_scope();
        for m in methods {
            self.check_stmt(*m);
        }
        self.pop_scope_and_check_unused();

        self.current_class = prev_class;
        self.generic_params_in_scope = prev_generics;
    }
}
