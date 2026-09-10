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
                    _ => Ok(Ty::Int), // Simplified for MVP
                }
            }
            Expr::MemberAccess { object, member, .. } => {
                let obj_ty = self.check_expr(object)?;
                match obj_ty {
                    Ty::Struct(_) | Ty::Class(_) => {
                        // For v0.1 we don't store fields in Type Env, so we mock it.
                        // We just assume the member access is an Int for now.
                        Ok(Ty::Int)
                    }
                    _ => Err(format!("Cannot access member '{}' on type {:?}", member, obj_ty)),
                }
            }
        }
    }
}
