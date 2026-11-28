use super::ResolvedField;
use pace_hir::{Decl, Expr, HirId, Stmt};

use crate::ty::Ty;
use pace_errors::{Diagnostic, ErrorCode};

use super::TypeChecker;

impl TypeChecker {
    /// Returns true for compiler-synthesized methods (on named types) and
    /// monomorphized generic declarations so lint passes do not warn on them.
    pub(super) fn is_compiler_generated_name(&self, id: HirId, name: &str) -> bool {
        self.named_types.keys().any(|k| {
            name.starts_with(k)
                || k.split('_')
                    .next_back()
                    .is_some_and(|short_k| name.starts_with(&format!("{}_", short_k)))
        }) || self.display_names.contains_key(name)
            || self
                .resolved_global_names
                .get(&id)
                .is_some_and(|m| self.display_names.contains_key(m))
    }

    pub fn check_decl(&mut self, decl: &Decl) -> Result<(), String> {
        match decl {
            &pace_hir::Decl::Import { .. } => Ok(()),
            Decl::Let {
                id,
                name,
                ty: explicit_ty,
                value,
                is_private: _,
                span,
            } => {
                let mut expected_ty = None;
                if let Some(explicit) = explicit_ty {
                    expected_ty = Some(self.resolve_type(explicit)?);
                }

                let mut ty = if let Some(val) = value {
                    self.initialized_bindings.insert(*id);
                    let prev_expected = self.current_expected_ty.take();
                    self.current_expected_ty = expected_ty.clone();
                    let res = self.check_expr(val);
                    self.current_expected_ty = prev_expected;
                    res?
                } else {
                    let expected = match expected_ty.clone() {
                        Some(ty) => ty,
                        None => {
                            self.reporter.report(
                                pace_errors::Diagnostic::error("Variable declaration must have a type or an initializer")
                                    .with_span(*span)
                                    .with_code(pace_errors::ErrorCode::TypeMismatch),
                            );
                            return Err("Variable declaration must have a type or an initializer".to_string());
                        }
                    };
                    if let Ty::Optional(_) = expected {
                        self.initialized_bindings.insert(*id);
                    }
                    expected
                };

                if let Some(expected) = expected_ty
                    && ty != expected
                {
                    if expected != Ty::Optional(Box::new(ty.clone())) {
                        self.reporter.report(
                            pace_errors::Diagnostic::error(format!(
                                "Type mismatch: expected {}, got {}",
                                self.display_ty(&expected),
                                self.display_ty(&ty)
                            ))
                            .with_span(explicit_ty.as_ref().map(|t| t.span()).unwrap_or(*span)),
                        );
                        return Err("Type mismatch".to_string());
                    }
                    ty = expected;
                }
                self.env.insert(*id, ty.clone());
                self.local_types.insert(*id, ty.clone());
                if let Some(mangled_name) = self.resolved_global_names.get(id).cloned() {
                    self.global_vars.insert(mangled_name, ty.clone());
                }
                self.mutability_env.insert(*id, false); // Let is immutable
                self.declared_bindings.push((*id, name.to_string(), *span));
                Ok(())
            }
            Decl::Var {
                id,
                name,
                ty: explicit_ty,
                value,
                is_private: _,
                span,
            } => {
                let mut ty = if let Some(val) = value {
                    self.initialized_bindings.insert(*id);
                    self.check_expr(val)?
                } else {
                    let explicit = match explicit_ty.as_ref() {
                        Some(t) => t,
                        None => {
                            self.reporter.report(
                                pace_errors::Diagnostic::error("Class field must have a type")
                                    .with_span(*span)
                                    .with_code(pace_errors::ErrorCode::TypeMismatch),
                            );
                            return Err("Class field must have a type".to_string());
                        }
                    };
                    let expected = self.resolve_type(explicit)?;
                    if let Ty::Optional(_) = expected {
                        self.initialized_bindings.insert(*id);
                    }
                    expected
                };

                if let Some(explicit) = explicit_ty {
                    let expected = self.resolve_type(explicit)?;
                    if ty != expected {
                        if expected != Ty::Optional(Box::new(ty.clone())) {
                            self.reporter.report(
                                pace_errors::Diagnostic::error(format!(
                                    "Type mismatch: expected {}, got {}",
                                    self.display_ty(&expected),
                                    self.display_ty(&ty)
                                ))
                                .with_span(explicit.span()),
                            );
                            return Err("Type mismatch".to_string());
                        }
                        ty = expected;
                    }
                }
                self.env.insert(*id, ty.clone());
                self.local_types.insert(*id, ty.clone());
                if let Some(mangled_name) = self.resolved_global_names.get(id).cloned() {
                    self.global_vars.insert(mangled_name, ty.clone());
                }
                self.mutability_env.insert(*id, true); // Var is mutable
                self.declared_bindings.push((*id, name.to_string(), *span));
                Ok(())
            }
            Decl::Const {
                id,
                name,
                ty: explicit_ty,
                value,
                is_private: _,
                span,
            } => {
                let mut ty = self.check_expr(value)?;
                if let Some(explicit) = explicit_ty {
                    let expected = self.resolve_type(explicit)?;
                    if ty != expected {
                        if expected != Ty::Optional(Box::new(ty.clone())) {
                            self.reporter.report(
                                pace_errors::Diagnostic::error(format!(
                                    "Type mismatch: expected {}, got {}",
                                    self.display_ty(&expected),
                                    self.display_ty(&ty)
                                ))
                                .with_span(explicit.span()),
                            );
                            return Err("Type mismatch".to_string());
                        }
                        ty = expected;
                    }
                }
                self.env.insert(*id, ty);
                self.mutability_env.insert(*id, false);
                self.declared_bindings.push((*id, name.to_string(), *span));
                Ok(())
            }
            Decl::Struct {
                id,
                generic_params,
                methods,
                static_fields,
                const_fields,
                ..
            } => {
                if generic_params.is_some() {
                    return Ok(());
                }
                self.env.insert(*id, Ty::Struct(*id));
                for pace_hir::HirStaticFieldDef {
                    ty: sf_ty,
                    value: sf_expr,
                    ..
                } in static_fields
                {
                    let expected_ty = self.resolve_type(sf_ty)?;
                    let expr_ty = self.check_expr(sf_expr)?;
                    if expected_ty != expr_ty {
                        self.reporter.report(
                            Diagnostic::error(format!(
                                "Type mismatch: expected {}, got {}",
                                self.display_ty(&expected_ty),
                                self.display_ty(&expr_ty)
                            ))
                            .with_span(sf_expr.span())
                            .with_code(ErrorCode::TypeMismatch),
                        );
                        return Err("Type mismatch".to_string());
                    }
                }
                for pace_hir::HirConstFieldDef {
                    ty: cf_ty,
                    value: cf_expr,
                    ..
                } in const_fields
                {
                    let expected_ty = self.resolve_type(cf_ty)?;
                    let expr_ty = self.check_expr(cf_expr)?;
                    if expected_ty != expr_ty {
                        self.reporter.report(
                            Diagnostic::error(format!(
                                "Type mismatch: expected {}, got {}",
                                self.display_ty(&expected_ty),
                                self.display_ty(&expr_ty)
                            ))
                            .with_span(cf_expr.span())
                            .with_code(ErrorCode::TypeMismatch),
                        );
                        return Err("Type mismatch".to_string());
                    }
                }
                for method in methods {
                    self.check_decl(method)?;
                }
                Ok(())
            }
            Decl::Class {
                id,
                generic_params,
                methods,
                static_fields,
                const_fields,
                ..
            } => {
                if generic_params.is_some() {
                    return Ok(());
                }
                self.env.insert(*id, Ty::Class(*id));
                for pace_hir::HirStaticFieldDef {
                    ty: sf_ty,
                    value: sf_expr,
                    ..
                } in static_fields
                {
                    let expected_ty = self.resolve_type(sf_ty)?;
                    let expr_ty = self.check_expr(sf_expr)?;
                    if expected_ty != expr_ty {
                        self.reporter.report(
                            Diagnostic::error(format!(
                                "Type mismatch: expected {}, got {}",
                                self.display_ty(&expected_ty),
                                self.display_ty(&expr_ty)
                            ))
                            .with_span(sf_expr.span())
                            .with_code(ErrorCode::TypeMismatch),
                        );
                        return Err("Type mismatch".to_string());
                    }
                }
                for pace_hir::HirConstFieldDef {
                    ty: cf_ty,
                    value: cf_expr,
                    ..
                } in const_fields
                {
                    let expected_ty = self.resolve_type(cf_ty)?;
                    let expr_ty = self.check_expr(cf_expr)?;
                    if expected_ty != expr_ty {
                        self.reporter.report(
                            Diagnostic::error(format!(
                                "Type mismatch: expected {}, got {}",
                                self.display_ty(&expected_ty),
                                self.display_ty(&expr_ty)
                            ))
                            .with_span(cf_expr.span())
                            .with_code(ErrorCode::TypeMismatch),
                        );
                        return Err("Type mismatch".to_string());
                    }
                }
                for method in methods {
                    self.check_decl(method)?;
                }
                Ok(())
            }
            Decl::Enum {
                id, generic_params, ..
            } => {
                if generic_params.is_some() {
                    return Ok(());
                }
                self.env.insert(*id, Ty::Enum(*id));
                Ok(())
            }
            Decl::Expr(expr, _) => {
                self.check_expr(expr)?;
                Ok(())
            }
            Decl::Function {
                id,
                name,
                generic_params,
                params,
                return_type,
                body,
                span,
                is_static,
                is_async,
                ..
            } => {
                if generic_params.is_some() {
                    self.generic_templates
                        .insert(name.to_string(), decl.clone());
                    self.generic_templates_by_id.insert(*id, name.to_string());
                    return Ok(());
                }
                
                let mangled_name = if let Some(m) = self.resolved_global_names.get(id) {
                    m.clone()
                } else {
                    format!("{}_{}", self.current_module.as_ref().unwrap_or(&"".to_string()), name)
                };
                
                let is_compiler_generated = self.is_compiler_generated_name(*id, name);

                if !is_compiler_generated {
                    let offset = if *is_static {
                        10
                    } else if *is_async {
                        9
                    } else {
                        3
                    };
                    let name_span = pace_span::Span::new(
                        span.file_id,
                        span.start + offset as u32,
                        span.start + (offset + name.len()) as u32,
                    );

                    if name.contains('_') && name != "main" && !name.ends_with("_init") {
                        self.reporter.report(
                            Diagnostic::warning(format!(
                                "Function '{}' should use camelCase, not snake_case",
                                name
                            ))
                            .with_span(name_span)
                            .with_code(ErrorCode::SnakeCaseName)
                            .with_hint("Rename to camelCase"),
                        );
                    }

                    self.declared_bindings
                        .push((*id, name.to_string(), name_span));
                }
                self.initialized_bindings.insert(*id);

                // Definite assignment check for initializers
                if name.ends_with("_init") {
                    let type_name = name.trim_end_matches("_init");
                    let mut assigned_fields = std::collections::HashSet::new();
                    let self_id = params
                        .iter()
                        .find(|(_, name, _)| name == "self")
                        .map(|(id, _, _)| *id);

                    // Simple analysis: collect all `self.field = value` assignments in the top-level block
                    for stmt in &body.statements {
                        if let Stmt::ExprStmt(Expr::Assign { target, .. }, _) = stmt
                            && let Expr::MemberAccess { object, member, .. } = &**target
                            && let Expr::Ident(obj_id, _, _, _) = &**object
                            && Some(*obj_id) == self_id
                        {
                            assigned_fields.insert(member.to_string());
                        }
                    }

                    if let Some(&(hir_id, kind)) = self.named_types.get(type_name) {
                        if kind == 1 {
                            let mut calls_super = false;
                            for stmt in &body.statements {
                                if let Stmt::ExprStmt(Expr::Call { callee, .. }, _) = stmt
                                    && let Expr::MemberAccess { object, .. } = &**callee
                                    && matches!(&**object, Expr::Super(_))
                                {
                                    calls_super = true;
                                }
                            }
                            if calls_super
                                && let Some(parent_id) = self.class_parents.get(&hir_id)
                                && let Some(parent_fields) = self.class_defs.get(parent_id)
                            {
                                for ResolvedField { name: fname, .. } in parent_fields {
                                    assigned_fields.insert(fname.clone());
                                }
                            }
                        }

                        let fields = if kind == 1 {
                            self.class_defs.get(&hir_id)
                        } else if kind == 0 {
                            self.struct_defs.get(&hir_id)
                        } else {
                            None
                        };

                        if let Some(fields) = fields {
                            for ResolvedField { name: fname, .. } in fields {
                                if !assigned_fields.contains(fname) {
                                    self.reporter.report(Diagnostic::error(format!("Field '{}' must be initialized", fname))
                                        .with_span(*span)
                                        .with_hint(format!("Assign a value to 'self.{}' in the init method, or initialize it inline", fname)));
                                    return Err("Definite assignment error".to_string());
                                }
                            }
                        }
                    }
                }

                let outer_env = self.env.clone();
                let prev_fn = self.current_fn_name.take();
                self.current_fn_name = Some(mangled_name.clone());
                for (param_id, param_name, pty) in params {
                    let ty = self.resolve_type(pty)?;
                    self.env.insert(*param_id, ty.clone());
                    self.local_types.insert(*param_id, ty);
                    self.declared_bindings
                        .push((*param_id, param_name.to_string(), *span));
                    self.initialized_bindings.insert(*param_id);
                }
                let ret_ty = if let Some(rty) = &return_type {
                    self.resolve_type(rty)?
                } else {
                    Ty::Void
                };

                let returns_exhaustively = self.check_exhaustive_return(body);
                let requires_return = ret_ty != Ty::Void
                    && ret_ty != Ty::Never
                    && !matches!(ret_ty, Ty::Future(ref inner) if **inner == Ty::Void || **inner == Ty::Never);
                if !returns_exhaustively && requires_return {
                    self.reporter.report(Diagnostic::error(format!("Function '{}' expects to return {}, but does not exhaustively return a value", name, self.display_ty(&ret_ty)))
                        .with_span(*span)
                        .with_code(ErrorCode::NonExhaustiveReturn));
                }

                let mut expected_block_ret = ret_ty.clone();
                let prev_async = self.in_async_context;
                if *is_async {
                    self.in_async_context = true;
                    if let Ty::Future(inner_ty) = &ret_ty {
                        expected_block_ret = (**inner_ty).clone();
                    } else {
                        self.reporter.report(Diagnostic::error(format!("Async function '{}' must return Future<T>", name))
                            .with_span(*span));
                    }
                }

                let prev_ret = self.current_return_ty.take();
                self.current_return_ty = Some(expected_block_ret.clone());

                self.check_block(body, Some(&expected_block_ret))?;
                
                self.current_return_ty = prev_ret;
                self.in_async_context = prev_async;
                self.current_fn_name = prev_fn;
                self.env = outer_env;
                Ok(())
            }
            Decl::Actor {
                id,
                generic_params,
                methods,
                ..
            } => {
                if generic_params.is_some() {
                    return Ok(());
                }
                self.env.insert(*id, Ty::Actor(*id));
                for method in methods {
                    self.check_decl(method)?;
                }
                Ok(())
            }
            Decl::Trait { generic_params, .. } => {
                if generic_params.is_some() {
                    return Ok(());
                }
                // Trait methods are static mixins. They are typechecked
                // when they are injected into a Class or Struct.
                Ok(())
            }
        }
    }

}
