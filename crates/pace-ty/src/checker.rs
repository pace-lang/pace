use pace_ast::{Expr, Stmt, BinaryOp, Visibility};
use crate::env::{Environment, Type, FunctionSignature, ClassSignature, EnumSignature};
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
    current_module: String,
    generic_params_in_scope: Vec<String>,
    pub warnings: Vec<pace_errors::SemanticWarning>,
    pub errors: Vec<TypeError>,
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            env: Environment::new(),
            current_return_type: None,
            current_class: None,
            current_module: "main".to_string(),
            generic_params_in_scope: Vec::new(),
            warnings: Vec::new(),
            errors: Vec::new(),
        }
    }

    pub fn check(&mut self, stmts: &[Stmt]) {
        // Pass 1: Hoisting
        if let Err(e) = self.hoist_declarations(stmts) {
            self.errors.push(e);
        }

        // Pass 2: Checking bodies
        for stmt in stmts {
            if let Err(e) = self.check_stmt(stmt) {
                self.errors.push(e);
            }
        }
        
        self.pop_scope_and_check_unused();
    }

    fn pop_scope_and_check_unused(&mut self) {
        let unused = self.env.pop_scope();
        for (name, var_info) in unused {
            if !var_info.is_used && !name.starts_with('_') && name != "self" {
                let kind = if var_info.ty == Type::Function {
                    "function"
                } else {
                    "variable"
                };
                self.warnings.push(pace_errors::SemanticWarning::UnusedItem {
                    kind: kind.to_string(),
                    name,
                    src: miette::NamedSource::new("", String::new()),
                    span: var_info.span,
                });
            }
        }
    }

    fn hoist_declarations(&mut self, stmts: &[Stmt]) -> Result<(), TypeError> {
        // Pre-register names so resolve_type_name knows what is a class/struct/interface
        for stmt in stmts {
            match stmt {
                Stmt::Module { name, body } => {
                    let old_module = self.current_module.clone();
                    self.current_module = name.clone();
                    self.hoist_declarations(body)?;
                    self.current_module = old_module;
                }
                Stmt::ClassDecl { name, .. } | Stmt::InterfaceDecl { name, .. } => {
                    self.env.classes.insert(name.clone(), ClassSignature { generic_params: None, fields: HashMap::new(), methods: HashMap::new() });
                }
                Stmt::StructDecl { name, .. } => {
                    self.env.structs.insert(name.clone(), ClassSignature { generic_params: None, fields: HashMap::new(), methods: HashMap::new() });
                }
                Stmt::EnumDecl { name, .. } => {
                    self.env.enums.insert(name.clone(), EnumSignature { generic_params: None, variants: HashMap::new() });
                }
                Stmt::Import { path, .. } => {
                    if path == "std/collection" {
                        self.env.register_class("List".to_string(), ClassSignature {
                            generic_params: Some(vec!["T".to_string()]),
                            fields: HashMap::new(),
                            methods: HashMap::new(),
                        });
                        self.env.register_class("Set".to_string(), ClassSignature {
                            generic_params: Some(vec!["T".to_string()]),
                            fields: HashMap::new(),
                            methods: HashMap::new(),
                        });
                    }
                    if path == "std/string" {
                        self.env.register_class("String".to_string(), ClassSignature { generic_params: None, fields: HashMap::new(), methods: HashMap::new() });
                    }
                    if path == "std/io" {
                        self.env.register_class("File".to_string(), ClassSignature { generic_params: None, fields: HashMap::new(), methods: HashMap::new() });
                    }
                }
                _ => {}
            }
        }

        for stmt in stmts {
            match stmt {
                Stmt::FuncDecl { name, params, return_type, span, visibility, generic_params, .. } => {
                    let mut param_types = Vec::new();
                    for param in params {
                        param_types.push(self.resolve_type_name(&param.type_annotation));
                    }
                    let ret_ty = if let Some(rt) = return_type {
                        self.resolve_type_name(rt)
                    } else {
                        Type::Void
                    };
                    if !is_camel_case(name) && name != "main" {
                        self.warnings.push(pace_errors::SemanticWarning::NamingConvention {
                            name: name.clone(),
                            src: miette::NamedSource::new("", String::new()),
                            span: *span,
                        });
                    }
                    let sig = FunctionSignature {
                        params: param_types,
                        return_type: ret_ty,
                        span: *span,
                        is_used: false,
                        visibility: visibility.clone(),
                        module: self.current_module.clone(),
                        generic_params: generic_params.clone(),
                    };
                    self.env.register_function(name.clone(), sig);
                }
                Stmt::ClassDecl { name, fields, methods, generic_params, .. } => {
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
                        if let Stmt::FuncDecl { name: m_name, params, return_type, visibility, .. } = m {
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
                                span: (0, 0),
                                is_used: true,
                                visibility: visibility.clone(),
                                module: self.current_module.clone(),
                                generic_params: None, // Methods inherit class generics, or have their own (TODO)
                            };
                            method_map.insert(m_name.clone(), sig);
                        }
                    }

                    let sig = ClassSignature {
                        generic_params: generic_params.clone(),
                        fields: field_map,
                        methods: method_map,
                    };
                    self.env.register_class(name.clone(), sig);
                }
                Stmt::StructDecl { name, fields, generic_params } => {
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
                        generic_params: generic_params.clone(),
                        fields: field_map,
                        methods: HashMap::new(),
                    };
                    self.env.register_struct(name.clone(), sig);
                }
                Stmt::EnumDecl { name, variants, generic_params } => {
                    let mut variant_map = HashMap::new();
                    self.current_class = Some(name.clone());
                    
                    if let Some(params) = generic_params {
                        self.env.push_scope();
                        for param in params {
                            self.env.define(param.clone(), Type::GenericParameter(param.clone()), (0, 0), false);
                        }
                    }
                    
                    for v in variants {
                        let fields = if let Some(fs) = &v.fields {
                            let mut resolved = Vec::new();
                            for f in fs {
                                resolved.push(self.resolve_type_name(f));
                            }
                            Some(resolved)
                        } else {
                            None
                        };
                        variant_map.insert(v.name.clone(), fields);
                    }
                    
                    if generic_params.is_some() {
                        self.env.pop_scope();
                    }
                    
                    self.current_class = None;
                    
                    let sig = EnumSignature {
                        generic_params: generic_params.clone(),
                        variants: variant_map,
                    };
                    self.env.register_enum(name.clone(), sig);
                }
                Stmt::InterfaceDecl { name, methods, generic_params } => {
                    let mut method_map = HashMap::new();
                    for m in methods {
                        if let Stmt::FuncDecl { name: m_name, params, return_type, visibility, .. } = m {
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
                                span: (0, 0),
                                is_used: true,
                                visibility: visibility.clone(),
                                module: self.current_module.clone(),
                                generic_params: None,
                            };
                            method_map.insert(m_name.clone(), sig);
                        }
                    }
                    let sig = ClassSignature {
                        generic_params: generic_params.clone(),
                        fields: HashMap::new(),
                        methods: method_map,
                    };
                    self.env.register_class(name.clone(), sig);
                }
                Stmt::Import { path, items: _ }
                    // Basic placeholder for module resolution.
                    // For now, if we import "std/collection", we mock registering `List` and `Set`
                    if path == "std/collection" => {
                        self.env.register_class("List".to_string(), ClassSignature {
                            generic_params: Some(vec!["T".to_string()]),
                            fields: HashMap::new(),
                            methods: HashMap::new(),
                        });
                        self.env.register_class("Set".to_string(), ClassSignature {
                            generic_params: Some(vec!["T".to_string()]),
                            fields: HashMap::new(),
                            methods: HashMap::new(),
                        });
                    }
                _ => {}
            }
        }
        Ok(())
    }

    fn check_stmt(&mut self, stmt: &Stmt) -> Result<(), TypeError> {
        match stmt {
            Stmt::Module { name, body } => {
                let old = self.current_module.clone();
                self.current_module = name.clone();
                for s in body {
                    self.check_stmt(s)?;
                }
                self.current_module = old;
            }
            Stmt::Expr(expr) => {
                self.check_expr(expr)?;
            }
            Stmt::VarDecl { name, is_mutable, type_annotation, initializer, span, .. } => {
                let mut inferred_type = Type::Unknown;
                
                if let Some(init_expr) = initializer {
                    inferred_type = self.check_expr(init_expr)?;
                }
                
                if let Some(annotation) = type_annotation {
                    let expected_type = self.resolve_type_name(annotation);
                    if inferred_type != Type::Unknown && inferred_type != expected_type {
                        self.env.define(name.clone(), expected_type.clone(), *span, *is_mutable);
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
                
                if !is_camel_case(name) {
                    self.warnings.push(pace_errors::SemanticWarning::NamingConvention {
                        name: name.clone(),
                        src: miette::NamedSource::new("", String::new()),
                        span: *span,
                    });
                }
                self.env.define(name.clone(), inferred_type, *span, *is_mutable);
            }
            Stmt::Block(stmts) => {
                self.env.push_scope();
                for s in stmts {
                    if let Err(e) = self.check_stmt(s) {
                        self.errors.push(e);
                    }
                }
                self.pop_scope_and_check_unused();
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
                self.pop_scope_and_check_unused();
            }
            Stmt::Match { expr, arms } => {
                self.check_expr(expr)?;
                for (pattern, body) in arms {
                    self.check_expr(pattern)?;
                    self.check_stmt(body)?;
                }
            }
            Stmt::FuncDecl { name: _, params, body, return_type, generic_params, .. } => {
                let prev_return = self.current_return_type.clone();
                let prev_generics = self.generic_params_in_scope.clone();
                
                if let Some(gps) = generic_params {
                    self.generic_params_in_scope.extend(gps.clone());
                }
                
                let ret_ty = if let Some(rt) = return_type {
                    self.resolve_type_name(rt)
                } else {
                    Type::Void
                };
                self.current_return_type = Some(ret_ty);
                
                self.env.push_scope();
                
                // Add `self` if we are inside a class/struct
                if let Some(class_name) = &self.current_class {
                    let self_ty = if self.env.structs.contains_key(class_name) {
                        Type::Struct(class_name.clone())
                    } else {
                        Type::Class(class_name.clone())
                    };
                    self.env.define("self".to_string(), self_ty, (0, 0), false);
                }
                
                // Add parameters to scope
                for param in params {
                    let param_type = self.resolve_type_name(&param.type_annotation);
                    self.env.define(param.name.clone(), param_type, (0, 0), false);
                }
                
                // Check body
                for s in body {
                    if let Err(e) = self.check_stmt(s) {
                        self.errors.push(e);
                    }
                }
                
                self.pop_scope_and_check_unused();
                self.current_return_type = prev_return;
                self.generic_params_in_scope = prev_generics;
            }
            Stmt::ClassDecl { name, methods, implements, generic_params, .. } => {
                let prev_class = self.current_class.clone();
                let prev_generics = self.generic_params_in_scope.clone();
                
                self.current_class = Some(name.clone());
                
                if let Some(gps) = generic_params {
                    self.generic_params_in_scope.extend(gps.clone());
                }
                
                if let Some(iface_annotation) = implements {
                    let iface_name = &iface_annotation.name;
                    // Check if class actually implements the interface
                    if let Some(iface_sig) = self.env.classes.get(iface_name) {
                        let class_sig = self.env.classes.get(name).unwrap().clone();
                        for m_name in iface_sig.methods.keys() {
                            if let Some(_actual_sig) = class_sig.methods.get(m_name) {
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
                    if let Err(e) = self.check_stmt(m) {
                        self.errors.push(e);
                    }
                }
                self.pop_scope_and_check_unused();
                
                self.current_class = prev_class;
                self.generic_params_in_scope = prev_generics;
            }
            Stmt::InterfaceDecl { .. } => {}
            Stmt::StructDecl { .. } => {}
            Stmt::EnumDecl { .. } => {}
            Stmt::Import { .. } => {}
        }
        Ok(())
    }

    fn check_expr(&mut self, expr: &Expr) -> Result<Type, TypeError> {
        match expr {
            Expr::IntLiteral(_) => Ok(Type::Int),
            Expr::FloatLiteral(_) => Ok(Type::Float),
            Expr::StringLiteral(_) => Ok(Type::String),
            Expr::GenericInstantiation { callee, .. } => {
                self.check_expr(callee)
            }
            Expr::InterpolatedString(parts) => {
                for part in parts {
                    let ty = self.check_expr(part)?;
                    if ty != Type::String && ty != Type::Int && ty != Type::Float && ty != Type::Bool {
                        return Err(TypeError {
                            message: format!("Cannot interpolate value of type {:?}", ty)
                        });
                    }
                }
                Ok(Type::String)
            }
            Expr::BoolLiteral(_) => Ok(Type::Bool),
            Expr::Null => Ok(Type::Null),
            Expr::Identifier(name) => {
                if let Some(var_info) = self.env.get_mut(name) {
                    var_info.is_used = true;
                }
                match self.env.get(name) {
                    Some(ty) => Ok(ty.clone()),
                    None => {
                        // Check if it's a class/struct for instantiation
                        if self.env.structs.contains_key(name) {
                            Ok(Type::Struct(name.clone()))
                        } else if self.env.classes.contains_key(name) {
                            Ok(Type::Class(name.clone()))
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
                    BinaryOp::Less | BinaryOp::LessEq | BinaryOp::Greater | BinaryOp::GreaterEq => {
                        if left_ty == Type::Int || left_ty == Type::Float {
                            Ok(Type::Bool)
                        } else {
                            Err(TypeError {
                                message: "Relational operations require numeric types".to_string()
                            })
                        }
                    }
                    BinaryOp::And | BinaryOp::Or => {
                        if left_ty == Type::Bool {
                            Ok(Type::Bool)
                        } else {
                            Err(TypeError {
                                message: "Logical operations require boolean types".to_string()
                            })
                        }
                    }
                }
            }
            Expr::Assign { target, value } => {
                let val_ty = self.check_expr(value)?;
                
                if let Expr::Identifier(name) = &**target {
                    if let Some(var_info) = self.env.get_mut(name) {
                        if !var_info.is_mutable {
                            return Err(TypeError {
                                message: format!("Cannot assign to immutable variable '{}'", name)
                            });
                        }
                        
                        if var_info.ty != val_ty && var_info.ty != Type::Unknown && val_ty != Type::Unknown {
                            return Err(TypeError {
                                message: format!("Type mismatch: cannot assign {:?} to variable of type {:?}", val_ty, var_info.ty)
                            });
                        }
                        
                        var_info.is_used = true;
                        Ok(val_ty)
                    } else {
                        Err(TypeError {
                            message: format!("Undefined variable '{}'", name)
                        })
                    }
                } else if let Expr::MemberAccess { object, .. } = &**target {
                    let _obj_ty = self.check_expr(object)?;
                    // Simple validation for now - real validation needs class layout check
                    Ok(val_ty)
                } else {
                    Err(TypeError {
                        message: "Invalid assignment target".to_string()
                    })
                }
            }
            Expr::Call { callee, args } => {
                let callee_ty = self.check_expr(callee)?;
                
                let mut arg_types = Vec::new();
                for arg in args {
                    arg_types.push(self.check_expr(arg)?);
                }
                
                // If callee is a known class/struct, it's a constructor call
                if let Type::Class(name) = &callee_ty {
                    if let Some(_sig) = self.env.classes.get(name) {
                        return Ok(Type::Class(name.clone()));
                    }
                } else if let Type::Struct(name) = &callee_ty
                    && let Some(_sig) = self.env.structs.get(name) {
                        return Ok(Type::Struct(name.clone()));
                    }
                
                // If it's a function or method, we need its signature
                // Currently, callee_ty might just be Type::Unknown if it was a MemberAccess
                // So if we don't know the type, we just return Unknown.
                // In a perfect world, MemberAccess returns a FunctionSignature type.
                
                // For direct global function calls
                if let Expr::Identifier(func_name) = &**callee
                    && let Some(sig) = self.env.functions.get(func_name) {
                        if sig.visibility == Visibility::Private && sig.module != self.current_module {
                            return Err(TypeError {
                                message: format!("Function '{}' is private and cannot be accessed outside of module '{}'", func_name, sig.module)
                            });
                        }
                        if sig.params.len() != args.len() {
                            return Err(TypeError {
                                message: format!("Function '{}' expects {} arguments, got {}", func_name, sig.params.len(), args.len())
                            });
                        }
                        
                        for (i, arg_ty) in arg_types.iter().enumerate() {
                            let expected_ty = &sig.params[i];
                            if expected_ty != &Type::Any && expected_ty != arg_ty {
                                return Err(TypeError {
                                    message: format!("Type mismatch in argument {}: expected {:?}, got {:?}", i + 1, expected_ty, arg_ty)
                                });
                            }
                        }
                        return Ok(sig.return_type.clone());
                    }
                
                // For member access calls (e.g. self.client.get())
                // We'll trust that the member access validated the existence of the method.
                // Since our MemberAccess doesn't return FunctionSignature yet, we just return Unknown
                Ok(Type::Unknown)
            }
            Expr::MemberAccess { object, property, .. } => {
                let obj_ty = self.check_expr(object)?;
                
                let (class_name, sig) = match obj_ty {
                    Type::Class(ref name) => {
                        let sig = self.env.classes.get(name).ok_or_else(|| TypeError {
                            message: format!("Type '{}' is not defined", name)
                        })?;
                        (name, sig)
                    },
                    Type::Struct(ref name) => {
                        let sig = self.env.structs.get(name).ok_or_else(|| TypeError {
                            message: format!("Type '{}' is not defined", name)
                        })?;
                        (name, sig)
                    },
                    _ => {
                        return Err(TypeError {
                            message: format!("Cannot access property '{}' on non-object type", property)
                        });
                    }
                };
                
                if let Some(f_ty) = sig.fields.get(property) {
                    return Ok(f_ty.clone());
                }
                if let Some(m_sig) = sig.methods.get(property) {
                    if m_sig.visibility == Visibility::Private
                        && self.current_class.as_ref() != Some(class_name) {
                            return Err(TypeError {
                                message: format!("Method '{}' is private and cannot be accessed from outside class '{}'", property, class_name)
                            });
                        }
                    return Ok(m_sig.return_type.clone());
                }
                Err(TypeError {
                    message: format!("Property '{}' not found on type '{}'", property, class_name)
                })
            }
            Expr::Unwrap(inner) => {
                let inner_ty = self.check_expr(inner)?;
                if let Type::Nullable(t) = inner_ty {
                    Ok(*t)
                } else {
                    Err(TypeError { message: "Cannot unwrap a non-nullable type".to_string() })
                }
            }
            Expr::NullCoalesce { left, right } => {
                let left_ty = self.check_expr(left)?;
                let right_ty = self.check_expr(right)?;
                if let Type::Nullable(inner) = left_ty {
                    if *inner == right_ty {
                        Ok(*inner)
                    } else if right_ty == Type::Null {
                        Ok(Type::Nullable(inner))
                    } else {
                        Err(TypeError { message: format!("Null coalesce type mismatch: {:?} and {:?}", *inner, right_ty) })
                    }
                } else {
                    Err(TypeError { message: "Left side of ?? must be nullable".to_string() })
                }
            }
            Expr::OptionalMemberAccess { object, property } => {
                let obj_ty = self.check_expr(object)?;
                if let Type::Nullable(inner) = obj_ty {
                    // Check property on inner type
                    let _inner_expr = Expr::MemberAccess {
                        object: Box::new(Expr::Null), // Dummy object to bypass recursive check_expr if we extracted logic
                        property: property.clone(),
                        computed_class: None,
                    };
                    // Instead of full check, we can manually check if it's Class or Struct
                    let (class_name, sig) = match &*inner {
                        Type::Class(name) => (name, self.env.classes.get(name).unwrap()),
                        Type::Struct(name) => (name, self.env.structs.get(name).unwrap()),
                        _ => return Err(TypeError { message: "Optional access on non-object".to_string() }),
                    };
                    
                    if let Some(f_ty) = sig.fields.get(property) {
                        return Ok(Type::Nullable(Box::new(f_ty.clone())));
                    }
                    if let Some(m_sig) = sig.methods.get(property) {
                        return Ok(Type::Nullable(Box::new(m_sig.return_type.clone())));
                    }
                    Err(TypeError { message: format!("Property '{}' not found on type '{}'", property, class_name) })
                } else {
                    Err(TypeError { message: "Optional member access on non-nullable type".to_string() })
                }
            }
        }
    }

    fn resolve_type_name(&self, annotation: &pace_ast::TypeAnnotation) -> Type {
        let base_name = &annotation.name;

        let mut base_type = match base_name.as_str() {
            "Int" => Type::Int,
            "Float" => Type::Float,
            "String" => Type::String,
            "Bool" => Type::Bool,
            "Void" => Type::Void,
            "Self" => {
                if let Some(current) = &self.current_class {
                    if self.env.structs.contains_key(current) {
                        Type::Struct(current.clone())
                    } else if self.env.enums.contains_key(current) {
                        Type::Enum(current.clone())
                    } else {
                        Type::Class(current.clone())
                    }
                } else {
                    Type::Unknown // Self used outside a class/struct/enum context
                }
            }
            _ => {
                if self.generic_params_in_scope.contains(base_name) {
                    Type::GenericParameter(base_name.to_string())
                } else if self.env.structs.contains_key(base_name) {
                    Type::Struct(base_name.to_string())
                } else if self.env.enums.contains_key(base_name) {
                    Type::Enum(base_name.to_string())
                } else {
                    Type::Class(base_name.to_string())
                }
            }
        };

        if !annotation.args.is_empty() {
            let mut arg_types = Vec::new();
            for arg in &annotation.args {
                arg_types.push(self.resolve_type_name(arg));
            }
            base_type = Type::GenericInstance { base: Box::new(base_type), args: arg_types };
        }

        if annotation.is_nullable {
            Type::Nullable(Box::new(base_type))
        } else {
            base_type
        }
    }
}

fn is_camel_case(s: &str) -> bool {
    if s.is_empty() { return true; }
    let first = s.chars().next().unwrap();
    if first.is_uppercase() { return false; }
    !s.contains('_')
}
