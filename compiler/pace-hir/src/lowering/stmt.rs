use crate::hir::*;
use pace_ast as ast;

use super::LoweringContext;

impl LoweringContext {
    pub fn lower_block(&mut self, block: ast::Block) -> Result<Block, String> {
        let mut statements = Vec::new();
        let outer_scope = self.scope.clone();
        for stmt in block.statements {
            match stmt {
                ast::Stmt::Let {
                    name,
                    ty,
                    value,
                    span,
                } => {
                    let id = self.generate_id();
                    let lowered_val = match value {
                        Some(expr) => Some(self.lower_expr(expr)?),
                        None => None,
                    };
                    self.bind_local(name.name, id);
                    statements.push(Stmt::Let {
                        id,
                        name: name.name,
                        ty,
                        value: lowered_val,
                        span,
                    });
                }
                ast::Stmt::Var {
                    name,
                    ty,
                    value,
                    span,
                } => {
                    let id = self.generate_id();
                    let lowered_val = match value {
                        Some(expr) => Some(self.lower_expr(expr)?),
                        None => None,
                    };
                    self.bind_local(name.name, id);
                    statements.push(Stmt::Var {
                        id,
                        name: name.name,
                        ty,
                        value: lowered_val,
                        span,
                    });
                }
                ast::Stmt::ExprStmt(expr, span) => {
                    statements.push(Stmt::ExprStmt(self.lower_expr(expr)?, span));
                }
                ast::Stmt::Return(expr, span) => {
                    let lowered_expr = match expr {
                        Some(e) => Some(self.lower_expr(e)?),
                        None => None,
                    };
                    statements.push(Stmt::Return(lowered_expr, span));
                }
                ast::Stmt::Break(span) => {
                    statements.push(Stmt::Break(span));
                }
                ast::Stmt::Continue(span) => {
                    statements.push(Stmt::Continue(span));
                }
                ast::Stmt::For {
                    var,
                    iter,
                    body,
                    span,
                } => {
                    let lowered_iter = self.lower_expr(iter)?;
                    let iter_id = self.generate_id();
                    let iter_sym = pace_span::intern(&format!("__iter_{}", iter_id.0));
                    self.bind_local(iter_sym, iter_id);
                    statements.push(Stmt::Let {
                        id: iter_id,
                        name: iter_sym,
                        ty: None,
                        value: Some(lowered_iter),
                        span,
                    });

                    let idx_id = self.generate_id();
                    let idx_sym = pace_span::intern(&format!("__idx_{}", idx_id.0));
                    self.bind_local(idx_sym, idx_id);
                    statements.push(Stmt::Var {
                        id: idx_id,
                        name: idx_sym,
                        ty: None,
                        value: Some(Expr::IntLiteral("0".to_string(), span)),
                        span,
                    });

                    let len_call = Expr::Call {
                        callee: Box::new(Expr::MemberAccess {
                            object: Box::new(Expr::Ident(iter_id, iter_sym, None, span)),
                            member: pace_span::intern("len"),
                            span,
                        }),
                        args: vec![],
                        span,
                    };
                    let cond = Expr::Binary {
                        left: Box::new(Expr::Ident(idx_id, idx_sym, None, span)),
                        op: pace_ast::BinaryOp::Lt,
                        right: Box::new(len_call),
                        span,
                    };

                    let elem_access = Expr::Index {
                        target: Box::new(Expr::Ident(iter_id, iter_sym, None, span)),
                        index: Box::new(Expr::Ident(idx_id, idx_sym, None, span)),
                        span: var.span,
                    };

                    let loop_outer_scope = self.scope.clone();
                    let var_id = self.generate_id();
                    self.bind_local(var.name, var_id);
                    let mut loop_stmts = Vec::new();
                    loop_stmts.push(Stmt::Let {
                        id: var_id,
                        name: var.name,
                        ty: None,
                        value: Some(elem_access),
                        span: var.span,
                    });

                    let lowered_body = self.lower_block(body)?;
                    loop_stmts.extend(lowered_body.statements);

                    let inc_expr = Expr::Assign {
                        target: Box::new(Expr::Ident(idx_id, idx_sym, None, span)),
                        value: Box::new(Expr::Binary {
                            left: Box::new(Expr::Ident(idx_id, idx_sym, None, span)),
                            op: pace_ast::BinaryOp::Add,
                            right: Box::new(Expr::IntLiteral("1".to_string(), span)),
                            span,
                        }),
                        span,
                    };
                    loop_stmts.push(Stmt::ExprStmt(inc_expr, span));

                    self.scope = loop_outer_scope;

                    let while_stmt = Expr::While {
                        cond: Box::new(cond),
                        body: Block {
                            statements: loop_stmts,
                            span: span,
                        },
                        span,
                    };
                    statements.push(Stmt::ExprStmt(while_stmt, span));
                }
            }
        }
        self.scope = outer_scope;
        Ok(Block {
            statements,
            span: block.span,
        })
    }
}
