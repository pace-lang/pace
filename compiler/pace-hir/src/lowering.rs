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
            _ => Err("Unsupported AST expression node in HIR lowering".to_string()),
        }
    }
}
