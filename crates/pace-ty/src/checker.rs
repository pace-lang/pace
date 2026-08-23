use pace_ast::{Expr, Stmt, BinaryOp};
use crate::env::{Environment, Type};
use miette::Diagnostic;
use thiserror::Error;

#[derive(Error, Diagnostic, Debug)]
#[error("Type error: {message}")]
#[diagnostic(code(pace::type_error))]
pub struct TypeError {
    pub message: String,
}

pub struct TypeChecker {
    env: Environment,
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            env: Environment::new(),
        }
    }

    pub fn check(&mut self, stmts: &[Stmt]) -> Result<(), TypeError> {
        for stmt in stmts {
            self.check_stmt(stmt)?;
        }
        Ok(())
    }

    fn check_stmt(&mut self, stmt: &Stmt) -> Result<(), TypeError> {
        match stmt {
            Stmt::Expr(expr) => {
                self.check_expr(expr)?;
            }
            Stmt::VarDecl { name, type_annotation, initializer, .. } => {
                let mut inferred_type = Type::Unknown;
                
                // If there's an initializer, infer its type
                if let Some(init_expr) = initializer {
                    inferred_type = self.check_expr(init_expr)?;
                }
                
                // If there is an explicit type annotation, check if it matches
                if let Some(annotation) = type_annotation {
                    let expected_type = self.resolve_type_name(annotation);
                    if inferred_type != Type::Unknown && inferred_type != expected_type {
                        return Err(TypeError {
                            message: format!(
                                "Type mismatch: expected {:?}, found {:?}",
                                expected_type, inferred_type
                            )
                        });
                    }
                    inferred_type = expected_type;
                }
                
                if inferred_type == Type::Unknown {
                    return Err(TypeError {
                        message: format!("Cannot infer type for variable '{}'", name)
                    });
                }
                
                self.env.define(name.clone(), inferred_type);
            }
            Stmt::Block(stmts) => {
                self.env.push_scope();
                for s in stmts {
                    self.check_stmt(s)?;
                }
                self.env.pop_scope();
            }
            Stmt::Return(expr_opt) => {
                if let Some(expr) = expr_opt {
                    self.check_expr(expr)?;
                }
                // TODO: Verify return type matches current function's return type
            }
            Stmt::If { condition, then_branch, else_branch } => {
                let cond_ty = self.check_expr(condition)?;
                if cond_ty != Type::Bool {
                    return Err(TypeError {
                        message: "If condition must be a boolean".to_string()
                    });
                }
                self.check_stmt(then_branch)?;
                if let Some(else_b) = else_branch {
                    self.check_stmt(else_b)?;
                }
            }
            Stmt::While { condition, body } => {
                let cond_ty = self.check_expr(condition)?;
                if cond_ty != Type::Bool {
                    return Err(TypeError {
                        message: "While condition must be a boolean".to_string()
                    });
                }
                self.check_stmt(body)?;
            }
            Stmt::Loop { body } => {
                self.check_stmt(body)?;
            }
            Stmt::ForIn { iterable, body, .. } => {
                // In a real compiler, we check if iterable is indeed iterable
                self.check_expr(iterable)?;
                // Push scope and define the item variable
                self.env.push_scope();
                // We fake the item type for now until we have generics/iterables
                self.check_stmt(body)?;
                self.env.pop_scope();
            }
            Stmt::Match { expr, arms } => {
                self.check_expr(expr)?;
                for (pattern, body) in arms {
                    self.check_expr(pattern)?;
                    self.check_stmt(body)?;
                }
            }
            Stmt::FuncDecl { name, params, body, .. } => {
                // In a real compiler we register the function signature first
                // Then typecheck the body in a new scope
                self.env.define(name.clone(), Type::Custom("Function".to_string()));
                self.env.push_scope();
                
                // Add parameters to the environment
                for param in params {
                    let param_type = self.resolve_type_name(&param.type_annotation);
                    self.env.define(param.name.clone(), param_type);
                }
                
                for s in body {
                    self.check_stmt(s)?;
                }
                self.env.pop_scope();
            }
            Stmt::ClassDecl { name, fields, methods, .. } => {
                self.env.define(name.clone(), Type::Custom(name.clone()));
                self.env.push_scope();
                for f in fields {
                    self.check_stmt(f)?;
                }
                for m in methods {
                    self.check_stmt(m)?;
                }
                self.env.pop_scope();
            }
            Stmt::InterfaceDecl { name, .. } => {
                self.env.define(name.clone(), Type::Custom(name.clone()));
            }
            Stmt::StructDecl { name, fields } => {
                self.env.define(name.clone(), Type::Custom(name.clone()));
                self.env.push_scope();
                for f in fields {
                    self.check_stmt(f)?;
                }
                self.env.pop_scope();
            }
        }
        Ok(())
    }

    fn check_expr(&mut self, expr: &Expr) -> Result<Type, TypeError> {
        match expr {
            Expr::IntLiteral(_) => Ok(Type::Int),
            Expr::FloatLiteral(_) => Ok(Type::Float),
            Expr::StringLiteral(_) => Ok(Type::String),
            Expr::BoolLiteral(_) => Ok(Type::Bool),
            Expr::Null => Ok(Type::Null),
            Expr::Identifier(name) => {
                match self.env.get(name) {
                    Some(ty) => Ok(ty.clone()),
                    None => Err(TypeError {
                        message: format!("Undefined variable '{}'", name)
                    }),
                }
            }
            Expr::Binary { left, op, right } => {
                let left_ty = self.check_expr(left)?;
                let right_ty = self.check_expr(right)?;
                
                if left_ty != right_ty {
                    return Err(TypeError {
                        message: format!("Type mismatch in binary operation: {:?} and {:?}", left_ty, right_ty)
                    });
                }
                
                match op {
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => {
                        if left_ty == Type::Int || left_ty == Type::Float {
                            Ok(left_ty)
                        } else {
                            Err(TypeError {
                                message: "Arithmetic operations require numeric types".to_string()
                            })
                        }
                    }
                    BinaryOp::Eq | BinaryOp::NotEq => Ok(Type::Bool),
                }
            }
            Expr::Call { callee, args } => {
                let callee_ty = self.check_expr(callee)?;
                for arg in args {
                    self.check_expr(arg)?;
                }
                
                // If we are calling a Class/Struct constructor, the return type is that Class/Struct
                if let Type::Custom(name) = callee_ty {
                    if name != "Function" {
                        return Ok(Type::Custom(name));
                    }
                }
                
                // For now, normal functions return Unknown as we don't track function return types yet
                Ok(Type::Unknown)
            }
            Expr::MemberAccess { object, property: _ } => {
                self.check_expr(object)?;
                Ok(Type::Unknown)
            }
        }
    }

    fn resolve_type_name(&self, name: &str) -> Type {
        match name {
            "Int" => Type::Int,
            "Float" => Type::Float,
            "String" => Type::String,
            "Bool" => Type::Bool,
            _ => Type::Custom(name.to_string()),
        }
    }
}
