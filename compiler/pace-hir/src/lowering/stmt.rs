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
            }
        }
        self.scope = outer_scope;
        Ok(Block {
            statements,
            span: block.span,
        })
    }
}
