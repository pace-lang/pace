use super::LoweringContext;
use crate::hir::*;
use pace_ast as ast;

impl LoweringContext {
    pub(crate) fn lower_expr(&mut self, expr: ast::Expr) -> Result<Expr, String> {
        match expr {
            ast::Expr::IntLiteral(val, span) => Ok(Expr::IntLiteral(val, span)),
            ast::Expr::FloatLiteral(val, span) => Ok(Expr::FloatLiteral(val, span)),
            ast::Expr::StringLiteral(val, span) => Ok(Expr::StringLiteral(val, span)),
            ast::Expr::Ident(ident, generic_args) => {
                if ident.name == "true" {
                    return Ok(Expr::BoolLiteral(true, ident.span));
                }
                if ident.name == "false" {
                    return Ok(Expr::BoolLiteral(false, ident.span));
                }
                let id = if let Some(id) = self.scope.get(&ident.name).copied() {
                    id
                } else {
                    let id = self.generate_id();
                    self.scope.insert(ident.name.clone(), id);
                    id
                };
                let lowered_args =
                    generic_args.map(|args| args.into_iter().map(|arg| arg.clone()).collect());
                Ok(Expr::Ident(
                    id,
                    ident.name.clone(),
                    lowered_args,
                    ident.span,
                ))
            }
            ast::Expr::Super(span) => Ok(Expr::Super(span)),
            ast::Expr::Binary {
                left,
                op,
                right,
                span,
            } => Ok(Expr::Binary {
                left: Box::new(self.lower_expr(*left)?),
                op,
                right: Box::new(self.lower_expr(*right)?),
                span,
            }),
            ast::Expr::Null(span) => Ok(Expr::Null(span)),
            ast::Expr::MemberAccess {
                object,
                member,
                span,
            } => Ok(Expr::MemberAccess {
                object: Box::new(self.lower_expr(*object)?),
                member: member.name,
                span,
            }),
            ast::Expr::OptionalMemberAccess {
                object,
                member,
                span,
            } => Ok(Expr::OptionalMemberAccess {
                object: Box::new(self.lower_expr(*object)?),
                member: member.name,
                span,
            }),
            ast::Expr::Call { callee, args, span } => {
                if let ast::Expr::Ident(ident, _) = &*callee {
                    if ident.name == "print" || ident.name == "println" {
                        let mut lowered_args = Vec::new();
                        for (_, arg) in args {
                            lowered_args.push(self.lower_expr(arg)?);
                        }
                        return Ok(Expr::BuiltinCall(ident.name.clone(), lowered_args, span));
                    }
                }

                let mut lowered_args = Vec::new();
                for (label, arg) in args {
                    let lowered_label = label.map(|id| id.name);
                    lowered_args.push((lowered_label, self.lower_expr(arg)?));
                }
                Ok(Expr::Call {
                    callee: Box::new(self.lower_expr(*callee)?),
                    args: lowered_args,
                    span,
                })
            }
            ast::Expr::If {
                cond,
                then_block,
                else_block,
                span,
            } => {
                let lowered_cond = self.lower_expr(*cond)?;
                let lowered_then = self.lower_block(then_block)?;
                let lowered_else = match else_block {
                    Some(b) => Some(self.lower_block(b)?),
                    None => None,
                };
                Ok(Expr::If {
                    cond: Box::new(lowered_cond),
                    then_block: lowered_then,
                    else_block: lowered_else,
                    span,
                })
            }
            ast::Expr::While { cond, body, span } => {
                let lowered_cond = self.lower_expr(*cond)?;
                let lowered_body = self.lower_block(body)?;
                Ok(Expr::While {
                    cond: Box::new(lowered_cond),
                    body: lowered_body,
                    span,
                })
            }
            ast::Expr::Assign {
                target,
                value,
                span,
            } => {
                let lowered_target = self.lower_expr(*target)?;
                let lowered_val = self.lower_expr(*value)?;
                Ok(Expr::Assign {
                    target: Box::new(lowered_target),
                    value: Box::new(lowered_val),
                    span,
                })
            }
            ast::Expr::Match {
                subject,
                arms,
                span,
            } => {
                let lowered_subject = self.lower_expr(*subject)?;
                let mut lowered_arms = Vec::new();
                for arm in arms {
                    let outer_scope = self.scope.clone();

                    let pattern = match arm.pattern {
                        ast::Pattern::Ident(ident) => {
                            let id = self.generate_id();
                            self.scope.insert(ident.name.clone(), id);
                            crate::hir::Pattern::Ident(id, ident.name, ident.span)
                        }
                        ast::Pattern::Variant {
                            name,
                            fields,
                            span: p_span,
                        } => {
                            let mut lowered_fields = None;
                            if let Some(f) = fields {
                                let mut lf = Vec::new();
                                for fname in f {
                                    let id = self.generate_id();
                                    self.scope.insert(fname.name.clone(), id);
                                    lf.push((id, fname.name, fname.span));
                                }
                                lowered_fields = Some(lf);
                            }
                            crate::hir::Pattern::Variant {
                                name: name.name,
                                fields: lowered_fields,
                                span: p_span,
                            }
                        }
                        ast::Pattern::CatchAll(s) => crate::hir::Pattern::CatchAll(s),
                    };

                    let body = self.lower_expr(arm.body)?;
                    self.scope = outer_scope;

                    lowered_arms.push(crate::hir::MatchArm {
                        pattern,
                        body,
                        span: arm.span,
                    });
                }

                Ok(Expr::Match {
                    subject: Box::new(lowered_subject),
                    arms: lowered_arms,
                    span,
                })
            }
        }
    }
}
