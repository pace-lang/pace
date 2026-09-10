use std::collections::HashMap;
use crate::hir::*;
use pace_ast as ast;

/// Lowers an AST (with string names) into HIR (with unique HirIds).
pub struct LoweringContext {
    next_id: u32,
    scope: HashMap<String, HirId>,
}

impl LoweringContext {
    pub fn new() -> Self {
        Self {
            next_id: 0,
            scope: HashMap::new(),
        }
    }

    fn generate_id(&mut self) -> HirId {
        let id = self.next_id;
        self.next_id += 1;
        HirId(id)
    }

    pub fn lower_program(&mut self, ast: ast::Program) -> Result<Program, String> {
        let mut declarations = Vec::new();
        for decl in ast.declarations {
            if let Some(d) = self.lower_decl(decl)? {
                declarations.push(d);
            }
        }
        Ok(Program { declarations })
    }

    pub fn lower_block(&mut self, block: ast::Block) -> Result<Block, String> {
        let mut statements = Vec::new();
        let outer_scope = self.scope.clone();
        for stmt in block.statements {
            match stmt {
                ast::Stmt::Let { name, value, span } => {
                    let id = self.generate_id();
                    let lowered_val = self.lower_expr(value)?;
                    self.scope.insert(name.name.clone(), id);
                    statements.push(Stmt::Let { id, name: name.name, value: lowered_val, span });
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
        Ok(Block { statements, span: block.span })
    }

    fn lower_decl(&mut self, decl: ast::Decl) -> Result<Option<Decl>, String> {
        match decl {
            ast::Decl::Let { name, value, span } => {
                let id = self.generate_id();
                let lowered_val = self.lower_expr(value)?;
                self.scope.insert(name.name.clone(), id);
                Ok(Some(Decl::Let { id, name: name.name, value: lowered_val, span }))
            }
            ast::Decl::Struct { name, span, .. } => {
                let id = self.generate_id();
                self.scope.insert(name.name.clone(), id);
                Ok(Some(Decl::Struct { id, name: name.name, span }))
            }
            ast::Decl::Class { name, span, .. } => {
                let id = self.generate_id();
                self.scope.insert(name.name.clone(), id);
                Ok(Some(Decl::Class { id, name: name.name, span }))
            }
            ast::Decl::Expr(expr, span) => {
                let lowered = self.lower_expr(expr)?;
                Ok(Some(Decl::Expr(lowered, span)))
            }
            _ => Ok(None),
        }
    }

    fn lower_expr(&mut self, expr: ast::Expr) -> Result<Expr, String> {
        match expr {
            ast::Expr::IntLiteral(val, span) => Ok(Expr::IntLiteral(val, span)),
            ast::Expr::StringLiteral(val, span) => Ok(Expr::StringLiteral(val, span)),
            ast::Expr::Ident(ident) => {
                let id = self.scope.get(&ident.name).copied().ok_or(format!("Undefined variable: {}", ident.name))?;
                Ok(Expr::Ident(id, ident.span))
            }
            ast::Expr::Binary { left, op, right, span } => {
                Ok(Expr::Binary {
                    left: Box::new(self.lower_expr(*left)?),
                    op,
                    right: Box::new(self.lower_expr(*right)?),
                    span,
                })
            }
            ast::Expr::MemberAccess { object, member, span } => {
                Ok(Expr::MemberAccess {
                    object: Box::new(self.lower_expr(*object)?),
                    member: member.name,
                    span,
                })
            }
            ast::Expr::Call { callee, args, span } => {
                if let ast::Expr::Ident(ident) = &*callee {
                    if ident.name == "print" || ident.name == "println" {
                        let mut lowered_args = Vec::new();
                        for arg in args {
                            lowered_args.push(self.lower_expr(arg)?);
                        }
                        return Ok(Expr::BuiltinCall(ident.name.clone(), lowered_args, span));
                    }
                }
                
                let mut lowered_args = Vec::new();
                for arg in args {
                    lowered_args.push(self.lower_expr(arg)?);
                }
                Ok(Expr::Call {
                    callee: Box::new(self.lower_expr(*callee)?),
                    args: lowered_args,
                    span,
                })
            }
            ast::Expr::If { cond, then_block, else_block, span } => {
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
            ast::Expr::Assign { target, value, span } => {
                let id = *self.scope.get(&target.name).ok_or(format!("Cannot reassign unbound variable '{}'", target.name))?;
                let lowered_val = self.lower_expr(*value)?;
                Ok(Expr::Assign {
                    target: id,
                    value: Box::new(lowered_val),
                    span,
                })
            }
            _ => Err("Unsupported AST expression node in HIR lowering".to_string()),
        }
    }
}
