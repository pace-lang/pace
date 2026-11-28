use crate::ty::Ty;
use pace_errors::{Diagnostic, ErrorCode};

use super::TypeChecker;

impl TypeChecker {
    pub fn check_block(
        &mut self,
        block: &pace_hir::Block,
        expected_ret_ty: Option<&Ty>,
    ) -> Result<(), String> {
        let outer_env = self.env.clone();
        for stmt in &block.statements {
            match stmt {
                pace_hir::Stmt::Let {
                    id,
                    name,
                    ty: explicit_ty,
                    value,
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
                        let inferred = res?;
                        if explicit_ty.is_none() && inferred != Ty::Void {
                            self.inlay_hints.push((*span, format!(": {}", self.display_ty(&inferred))));
                        }
                        inferred
                    } else {
                        // Uninitialized variable
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
                            self.initialized_bindings.insert(*id); // Implicitly initialized to null
                        }
                        expected
                    };

                    if let Some(expected) = expected_ty {
                        if !self.is_assignable(&expected, &ty) {
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
                    self.mutability_env.insert(*id, false); // Let is immutable
                    self.declared_bindings.push((*id, name.to_string(), *span));
                }
                pace_hir::Stmt::Var {
                    id,
                    name,
                    ty: explicit_ty,
                    value,
                    span,
                } => {
                    let mut ty = if let Some(val) = value {
                        self.initialized_bindings.insert(*id);
                        let inferred = self.check_expr(val)?;
                        if explicit_ty.is_none() && inferred != Ty::Void {
                            self.inlay_hints.push((*span, format!(": {}", self.display_ty(&inferred))));
                        }
                        inferred
                    } else {
                        // Uninitialized variable
                        let explicit = match explicit_ty.as_ref() {
                            Some(t) => t,
                            None => {
                                self.reporter.report(
                                    pace_errors::Diagnostic::error("Function parameter must have a type")
                                        .with_span(*span)
                                        .with_code(pace_errors::ErrorCode::TypeMismatch),
                                );
                                return Err("Function parameter must have a type".to_string());
                            }
                        };
                        let expected = self.resolve_type(explicit)?;
                        if let Ty::Optional(_) = expected {
                            self.initialized_bindings.insert(*id); // Implicitly initialized to null
                        }
                        expected
                    };

                    if let Some(explicit) = explicit_ty {
                        let expected = self.resolve_type(explicit)?;
                        let mut type_matches = ty == expected;
                        if !type_matches {
                            if expected == Ty::Optional(Box::new(ty.clone())) {
                                type_matches = true;
                            } else if let (Ty::Optional(_), Ty::Optional(inner_val)) =
                                (&expected, &ty)
                                && **inner_val == Ty::Void
                            {
                                type_matches = true;
                            }
                        }

                        if !type_matches {
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
                        ty = expected; // Promote to Optional
                    }
                    self.env.insert(*id, ty.clone());
                    self.local_types.insert(*id, ty.clone());
                    self.mutability_env.insert(*id, true); // Var is mutable
                    self.declared_bindings.push((*id, name.to_string(), *span));
                }
                pace_hir::Stmt::ExprStmt(expr, _) => {
                    self.check_expr(expr)?;
                }
                pace_hir::Stmt::Return(expr, span) => {
                    let ret_ty = if let Some(e) = expr {
                        self.check_expr(e)?
                    } else {
                        Ty::Void
                    };

                    let expected = expected_ret_ty.or(self.current_return_ty.as_ref());
                    if let Some(expected) = expected {
                        let mut type_matches = ret_ty == *expected;
                        if !type_matches {
                            if *expected == Ty::Optional(Box::new(ret_ty.clone())) {
                                type_matches = true;
                            } else if let (Ty::Optional(_), Ty::Optional(inner_val)) =
                                (expected, &ret_ty)
                                && **inner_val == Ty::Void
                            {
                                type_matches = true;
                            }
                        }
                        if !type_matches {
                            self.reporter.report(
                                Diagnostic::error(format!(
                                    "Type mismatch: function expects to return {}, but returned {}",
                                    self.display_ty(expected),
                                    self.display_ty(&ret_ty)
                                ))
                                .with_span(*span)
                                .with_code(ErrorCode::TypeMismatch),
                            );
                        }
                    } else if self.current_return_ty.is_none() {
                        self.current_return_ty = Some(ret_ty);
                    }
                }
                pace_hir::Stmt::Break(span) => {
                    if self.loop_depth == 0 {
                        self.reporter.report(
                            Diagnostic::error("Cannot 'break' outside of a loop")
                                .with_span(*span),
                        );
                        return Err("Cannot 'break' outside of a loop".to_string());
                    }
                }
                pace_hir::Stmt::Continue(span) => {
                    if self.loop_depth == 0 {
                        self.reporter.report(
                            Diagnostic::error("Cannot 'continue' outside of a loop")
                                .with_span(*span),
                        );
                        return Err("Cannot 'continue' outside of a loop".to_string());
                    }
                }
            }
        }
        self.env = outer_env;
        Ok(())
    }

    pub fn check_exhaustive_return(&self, block: &pace_hir::Block) -> bool {
        for stmt in &block.statements {
            match stmt {
                pace_hir::Stmt::Return(..) => return true,
                pace_hir::Stmt::ExprStmt(
                    pace_hir::Expr::If {
                        then_block,
                        else_block,
                        ..
                    },
                    _,
                ) => {
                    let then_returns = self.check_exhaustive_return(then_block);
                    let else_returns = else_block
                        .as_ref()
                        .is_some_and(|b| self.check_exhaustive_return(b));
                    if then_returns && else_returns {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

}
