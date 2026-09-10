use std::collections::HashMap;
use pace_hir::{Expr, Decl, Program, HirId};
use pace_ast::BinaryOp;
use crate::ty::Ty;

pub struct TypeChecker {
    pub env: HashMap<HirId, Ty>,
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            env: HashMap::new(),
        }
    }

    pub fn check_program(&mut self, program: &Program) -> Result<(), String> {
        for decl in &program.declarations {
            self.check_decl(decl)?;
        }
        Ok(())
    }

    pub fn check_block(&mut self, block: &pace_hir::Block) -> Result<(), String> {
        let outer_env = self.env.clone();
        for stmt in &block.statements {
            match stmt {
                pace_hir::Stmt::Let { id, value, .. } => {
                    let ty = self.check_expr(value)?;
                    self.env.insert(*id, ty);
                }
                pace_hir::Stmt::ExprStmt(expr, _) => {
                    self.check_expr(expr)?;
                }
                pace_hir::Stmt::Return(expr, _) => {
                    if let Some(e) = expr {
                        self.check_expr(e)?;
                    }
                }
            }
        }
        self.env = outer_env;
        Ok(())
    }

    pub fn check_decl(&mut self, decl: &Decl) -> Result<(), String> {
        match decl {
            Decl::Let { id, value, .. } => {
                let ty = self.check_expr(value)?;
                self.env.insert(*id, ty);
                Ok(())
            }
            Decl::Struct { id, .. } => {
                self.env.insert(*id, Ty::Struct(*id));
                Ok(())
            }
            Decl::Class { id, .. } => {
                self.env.insert(*id, Ty::Class(*id));
                Ok(())
            }
            Decl::Expr(expr, _) => {
                self.check_expr(expr)?;
                Ok(())
            }
        }
    }

    fn check_expr(&mut self, expr: &Expr) -> Result<Ty, String> {
        match expr {
            Expr::IntLiteral(..) => Ok(Ty::Int),
            Expr::StringLiteral(..) => Ok(Ty::String),
            Expr::Ident(id, span) => {
                self.env.get(id).cloned().ok_or(format!("Cannot infer type for unbound variable at {:?}", span))
            }
            Expr::Binary { left, op, right, .. } => {
                let left_ty = self.check_expr(left)?;
                let right_ty = self.check_expr(right)?;

                match op {
                    pace_ast::BinaryOp::Add | pace_ast::BinaryOp::Sub |
                    pace_ast::BinaryOp::Mul | pace_ast::BinaryOp::Div => {
                        if left_ty == Ty::Int && right_ty == Ty::Int {
                            Ok(Ty::Int)
                        } else if left_ty == Ty::Float && right_ty == Ty::Float {
                            Ok(Ty::Float)
                        } else {
                            Err(format!("Type mismatch in binary operation: {:?} and {:?}", left_ty, right_ty))
                        }
                    }
                    pace_ast::BinaryOp::EqEq | pace_ast::BinaryOp::NotEq |
                    pace_ast::BinaryOp::Gt | pace_ast::BinaryOp::Lt |
                    pace_ast::BinaryOp::GtEq | pace_ast::BinaryOp::LtEq => {
                        Ok(Ty::Int) // Boolean represented as Int in MVP
                    }
                }
            }
            Expr::MemberAccess { object, member, .. } => {
                let obj_ty = self.check_expr(object)?;
                match obj_ty {
                    Ty::Struct(_) | Ty::Class(_) => {
                        Ok(Ty::Int)
                    }
                    _ => Err(format!("Cannot access member '{}' on type {:?}", member, obj_ty)),
                }
            }
            Expr::Call { callee, args, .. } => {
                self.check_expr(callee)?;
                for arg in args {
                    self.check_expr(arg)?;
                }
                Ok(Ty::Int) // Assume int return for now
            }
            Expr::BuiltinCall(name, args, _) => {
                let mut arg_types = Vec::new();
                for arg in args {
                    arg_types.push(self.check_expr(arg)?);
                }
                
                if name == "print" || name == "println" {
                    if arg_types.len() != 1 {
                        return Err(format!("{} takes exactly 1 argument", name));
                    }
                    Ok(Ty::Int)
                } else {
                    Err(format!("Unknown builtin: {}", name))
                }
            }
            Expr::If { cond, then_block, else_block, .. } => {
                self.check_expr(cond)?;
                self.check_block(then_block)?;
                if let Some(else_b) = else_block {
                    self.check_block(else_b)?;
                }
                Ok(Ty::Int)
            }
            Expr::While { cond, body, .. } => {
                self.check_expr(cond)?;
                self.check_block(body)?;
                Ok(Ty::Int)
            }
            Expr::Assign { target, value, .. } => {
                let target_ty = self.env.get(target).cloned().ok_or("Cannot reassign unbound variable")?;
                let val_ty = self.check_expr(value)?;
                if target_ty != val_ty {
                    return Err(format!("Type mismatch in assignment: expected {:?}, got {:?}", target_ty, val_ty));
                }
                Ok(target_ty)
            }
        }
    }
}
