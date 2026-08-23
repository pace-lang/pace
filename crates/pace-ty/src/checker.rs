use pace_ast::{Expr, Stmt, BinaryOp};
use crate::env::{Environment, Type, FunctionSignature, ClassSignature};
use miette::Diagnostic;
use thiserror::Error;
use std::collections::HashMap;

#[derive(Error, Diagnostic, Debug)]
#[error("Type error: {message}")]
#[diagnostic(code(pace::type_error))]
pub struct TypeError {
    pub message: String,
}

pub struct TypeChecker {
    env: Environment,
    current_return_type: Option<Type>,
    current_class: Option<String>,
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            env: Environment::new(),
            current_return_type: None,
            current_class: None,
        }
    }

    pub fn check(&mut self, stmts: &[Stmt]) -> Result<(), TypeError> {
        // Pass 1: Hoisting
        self.hoist_declarations(stmts)?;

        // Pass 2: Checking bodies
        for stmt in stmts {
            self.check_stmt(stmt)?;
        }
        Ok(())
    }

    fn hoist_declarations(&mut self, stmts: &[Stmt]) -> Result<(), TypeError> {
        for stmt in stmts {
            match stmt {
                Stmt::FuncDecl { name, params, return_type, .. } => {
                    let mut param_types = Vec::new();
                    for param in params {
                        param_types.push(self.resolve_type_name(&param.type_annotation));
                    }
                    let ret_ty = if let Some(rt) = return_type {
                        self.resolve_type_name(rt)
                    } else {
                        Type::Void
                    };
                    let sig = FunctionSignature {
                        params: param_types,
                        return_type: ret_ty,
                    };
                    self.env.register_function(name.clone(), sig);
                }
                Stmt::ClassDecl { name, fields, methods, .. } => {
                    let mut field_map = HashMap::new();
                    for f in fields {
                        if let Stmt::VarDecl { name: f_name, type_annotation, .. } = f {
                            let f_ty = if let Some(ty_str) = type_annotation {
                                self.resolve_type_name(ty_str)
                            } else {
                                Type::Unknown
                            };
                            field_map.insert(f_name.clone(), f_ty);
                        }
                    }

                    let mut method_map = HashMap::new();
                    for m in methods {
                        if let Stmt::FuncDecl { name: m_name, params, return_type, .. } = m {
                            let mut param_types = Vec::new();
                            for param in params {
                                param_types.push(self.resolve_type_name(&param.type_annotation));
                            }
                            let ret_ty = if let Some(rt) = return_type {
                                self.resolve_type_name(rt)
                            } else {
                                Type::Void
                            };
                            let sig = FunctionSignature {
                                params: param_types,
                                return_type: ret_ty,
                            };
                            method_map.insert(m_name.clone(), sig);
                        }
                    }

                    let sig = ClassSignature {
                        fields: field_map,
                        methods: method_map,
                    };
                    self.env.register_class(name.clone(), sig);
                }
                Stmt::StructDecl { name, fields } => {
                    let mut field_map = HashMap::new();
                    for f in fields {
                        if let Stmt::VarDecl { name: f_name, type_annotation, .. } = f {
                            let f_ty = if let Some(ty_str) = type_annotation {
                                self.resolve_type_name(ty_str)
                            } else {
                                Type::Unknown
                            };
                            field_map.insert(f_name.clone(), f_ty);
                        }
                    }
                    let sig = ClassSignature {
                        fields: field_map,
                        methods: HashMap::new(),
                    };
                    self.env.register_class(name.clone(), sig);
                }
                Stmt::InterfaceDecl { name, methods } => {
                    let mut method_map = HashMap::new();
                    for m in methods {
                        if let Stmt::FuncDecl { name: m_name, params, return_type, .. } = m {
                            let mut param_types = Vec::new();
                            for param in params {
                                param_types.push(self.resolve_type_name(&param.type_annotation));
                            }
                            let ret_ty = if let Some(rt) = return_type {
                                self.resolve_type_name(rt)
                            } else {
                                Type::Void
                            };
                            let sig = FunctionSignature {
                                params: param_types,
                                return_type: ret_ty,
                            };
                            method_map.insert(m_name.clone(), sig);
                        }
                    }
                    let sig = ClassSignature {
                        fields: HashMap::new(),
                        methods: method_map,
                    };
                    self.env.register_class(name.clone(), sig);
                }
                _ => {}
            }
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
                
                if let Some(init_expr) = initializer {
                    inferred_type = self.check_expr(init_expr)?;
                }
                
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
                let ret_ty = if let Some(expr) = expr_opt {
                    self.check_expr(expr)?
                } else {
                    Type::Void
                };
                
                if let Some(expected) = &self.current_return_type {
                    if expected != &ret_ty && expected != &Type::Unknown {
                        return Err(TypeError {
                            message: format!("Type mismatch: expected return type {:?}, found {:?}", expected, ret_ty)
                        });
                    }
                } else {
                    return Err(TypeError {
                        message: "Return statement outside of function".to_string()
                    });
                }
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
                self.check_expr(iterable)?;
                self.env.push_scope();
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
            Stmt::FuncDecl { name, params, body, return_type, .. } => {
                let prev_return = self.current_return_type.clone();
                
                let ret_ty = if let Some(rt) = return_type {
                    self.resolve_type_name(rt)
                } else {
                    Type::Void
                };
                self.current_return_type = Some(ret_ty);
                
                self.env.push_scope();
                
                // Add `self` if we are inside a class
                if let Some(class_name) = &self.current_class {
                    self.env.define("self".to_string(), Type::Custom(class_name.clone()));
                }
                
                for param in params {
                    let param_type = self.resolve_type_name(&param.type_annotation);
                    self.env.define(param.name.clone(), param_type);
                }
                
                for s in body {
                    self.check_stmt(s)?;
                }
                
                self.env.pop_scope();
                self.current_return_type = prev_return;
            }
            Stmt::ClassDecl { name, methods, implements, .. } => {
                let prev_class = self.current_class.clone();
                self.current_class = Some(name.clone());
                
                if let Some(iface_name) = implements {
                    // Check if class actually implements the interface
                    if let Some(iface_sig) = self.env.classes.get(iface_name) {
                        let class_sig = self.env.classes.get(name).unwrap().clone();
                        for (m_name, expected_sig) in &iface_sig.methods {
                            if let Some(actual_sig) = class_sig.methods.get(m_name) {
                                // For simplicity, we just check if it exists right now
                                // In a full compiler, we'd check parameter counts and types
                            } else {
                                return Err(TypeError {
                                    message: format!("Class '{}' does not implement method '{}' from interface '{}'", name, m_name, iface_name)
                                });
                            }
                        }
                    } else {
                        return Err(TypeError {
                            message: format!("Interface '{}' not found", iface_name)
                        });
                    }
                }
                
                self.env.push_scope();
                for m in methods {
                    self.check_stmt(m)?;
                }
                self.env.pop_scope();
                
                self.current_class = prev_class;
            }
            Stmt::InterfaceDecl { .. } => {}
            Stmt::StructDecl { .. } => {}
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
                    None => {
                        // Check if it's a class/struct for instantiation
                        if self.env.classes.contains_key(name) {
                            Ok(Type::Custom(name.clone()))
                        } else {
                            Err(TypeError {
                                message: format!("Undefined variable '{}'", name)
                            })
                        }
                    }
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
                
                let mut arg_types = Vec::new();
                for arg in args {
                    arg_types.push(self.check_expr(arg)?);
                }
                
                // If callee is a known class, it's a constructor call
                if let Type::Custom(name) = &callee_ty {
                    if let Some(_sig) = self.env.classes.get(name) {
                        return Ok(Type::Custom(name.clone()));
                    }
                }
                
                // If it's a function or method, we need its signature
                // Currently, callee_ty might just be Type::Unknown if it was a MemberAccess
                // So if we don't know the type, we just return Unknown.
                // In a perfect world, MemberAccess returns a FunctionSignature type.
                
                // For direct global function calls
                if let Expr::Identifier(func_name) = &**callee {
                    if let Some(sig) = self.env.functions.get(func_name) {
                        if sig.params.len() != args.len() {
                            return Err(TypeError {
                                message: format!("Function '{}' expects {} arguments, got {}", func_name, sig.params.len(), args.len())
                            });
                        }
                        // We could check each arg type here
                        return Ok(sig.return_type.clone());
                    }
                }
                
                // For member access calls (e.g. self.client.get())
                // We'll trust that the member access validated the existence of the method.
                // Since our MemberAccess doesn't return FunctionSignature yet, we just return Unknown
                Ok(Type::Unknown)
            }
            Expr::MemberAccess { object, property } => {
                let obj_ty = self.check_expr(object)?;
                
                if let Type::Custom(class_name) = obj_ty {
                    if let Some(sig) = self.env.classes.get(&class_name) {
                        if let Some(f_ty) = sig.fields.get(property) {
                            return Ok(f_ty.clone());
                        }
                        if let Some(m_sig) = sig.methods.get(property) {
                            // Ideally return the function signature as a Type
                            return Ok(m_sig.return_type.clone());
                        }
                        return Err(TypeError {
                            message: format!("Property '{}' not found on type '{}'", property, class_name)
                        });
                    } else {
                        return Err(TypeError {
                            message: format!("Type '{}' is not defined", class_name)
                        });
                    }
                }
                
                Err(TypeError {
                    message: format!("Cannot access property '{}' on non-object type", property)
                })
            }
        }
    }

    fn resolve_type_name(&self, name: &str) -> Type {
        match name {
            "Int" => Type::Int,
            "Float" => Type::Float,
            "String" => Type::String,
            "Bool" => Type::Bool,
            "Void" => Type::Void,
            _ => Type::Custom(name.to_string()),
        }
    }
}
