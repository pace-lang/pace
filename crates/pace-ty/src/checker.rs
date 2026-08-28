use pace_errors::TypeError;
use pace_ast::{Expr, Stmt, BinaryOp, Visibility};
use crate::env::{Environment, Type, FunctionSignature, ClassSignature, EnumSignature};

use std::collections::HashMap;

impl TypeChecker {
    pub fn get_span_for(&self, token: &str) -> (usize, usize) {
        if let Some(src) = self.sources.get(&self.current_module) {
            if let Some(idx) = src.find(token) {
                return (idx, token.len());
            }
        }
        (0, 0)
    }

    pub fn get_source(&self) -> miette::NamedSource<String> {
        let name = self.current_module.clone();
        let src = self.sources.get(&name).cloned().unwrap_or_default();
        miette::NamedSource::new(name, src)
    }
}
pub struct TypeChecker {
    pub file_name: String,
    pub sources: HashMap<String, String>,
    pub env: Environment,
    current_return_type: Option<Type>,
    current_class: Option<String>,
    current_module: String,
    generic_params_in_scope: Vec<String>,
    pub warnings: Vec<pace_errors::SemanticWarning>,
    pub errors: Vec<TypeError>,
    pub current_span: (usize, usize),
}

pub fn check(ast: &[Stmt], sources: HashMap<String, String>, entry_module: &str) -> (Vec<pace_errors::SemanticWarning>, Vec<TypeError>, Environment) {
    let mut checker = TypeChecker::new(sources, entry_module);
    
    // We set the initial current_module to the entry module
    checker.current_module = entry_module.to_string();
    checker.check(ast);
    (checker.warnings, checker.errors, checker.env)
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new(HashMap::new(), "")
    }
}

impl TypeChecker {
    pub fn new(sources: HashMap<String, String>, file_name: &str) -> Self {
        Self {
            file_name: file_name.to_string(),
            sources,
            env: Environment::new(),
            current_return_type: None,
            current_class: None,
            current_module: "main".to_string(),
            generic_params_in_scope: Vec::new(),
            warnings: Vec::new(),
            errors: Vec::new(),
            current_span: (0, 0),
        }
    }

    
    fn define_var(&mut self, name: String, ty: Type, span: (usize, usize), is_mutable: bool) {
        if let Err(original_span) = self.env.define(name.clone(), ty, span, is_mutable) {
            self.errors.push(TypeError::DuplicateDeclaration {
                name,
                src: self.get_source(),
                span,
                original_span,
            });
        }
    }

    pub fn check(&mut self, stmts: &[Stmt]) {
        // Pass 1: Register all types
        self.register_types(stmts);

        // Pass 2: Resolve all signatures
        self.resolve_signatures(stmts);

        // Pass 2: Checking bodies
        for stmt in stmts {
            self.check_stmt(stmt);
        }
        
        self.pop_scope_and_check_unused();
    }

    fn pop_scope_and_check_unused(&mut self) {
        let unused = self.env.pop_scope();
        for (name, var_info) in unused {
            if !var_info.is_used && !name.starts_with('_') && name != "self" {
                let kind = if matches!(var_info.ty, Type::Function { .. }) {
                    "function"
                } else {
                    "variable"
                };
                self.warnings.push(pace_errors::SemanticWarning::UnusedItem {
                    kind: kind.to_string(),
                    name,
                    src: self.get_source(),
                    span: var_info.span,
                });
            }
        }
    }

    fn register_types(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            match stmt {
                Stmt::Module { name, body } => {
                    let old_module = self.current_module.clone();
                    self.current_module = name.clone();
                    self.register_types(body);
                    self.current_module = old_module;
                }
                Stmt::ClassDecl { name, .. } | Stmt::InterfaceDecl { name, .. } => {
                    self.env.register_class(name.clone(), ClassSignature { generic_params: None, fields: HashMap::new(), static_fields: HashMap::new(), methods: HashMap::new() });
                }
                Stmt::ActorDecl { name, .. } => {
                    self.env.register_actor(name.clone(), crate::env::ActorSignature { generic_params: None, fields: HashMap::new(), static_fields: HashMap::new(), methods: HashMap::new() });
                }
                Stmt::StructDecl { name, .. } => {
                    self.env.register_struct(name.clone(), ClassSignature { generic_params: None, fields: HashMap::new(), static_fields: HashMap::new(), methods: HashMap::new() });
                }
                Stmt::EnumDecl { name, .. } => {
                    self.env.register_enum(name.clone(), EnumSignature { generic_params: None, variants: HashMap::new() });
                }
                _ => {}
            }
        }
    }

    fn resolve_signatures(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            match stmt {
                Stmt::Module { name, body } => {
                    let old_module = self.current_module.clone();
                    self.current_module = name.clone();
                    self.resolve_signatures(body);
                    self.current_module = old_module;
                }
                Stmt::FuncDecl { name, params, return_type, span, visibility, generic_params, is_static, .. } => {
                    let mut param_types = Vec::new();
                    for param in params {
                        param_types.push(self.resolve_type_name(&param.type_annotation));
                    }
                    let ret_ty = if let Some(rt) = return_type {
                        self.resolve_type_name(rt)
                    } else {
                        Type::Void
                    };
                    if !is_camel_case(name) && name != "main" && !name.contains("__") {
                        self.warnings.push(pace_errors::SemanticWarning::NamingConvention {
                            name: name.clone(),
                            src: self.get_source(),
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
                        is_static: *is_static,
                    };
                    self.env.register_function(name.clone(), sig);
                }
                Stmt::ClassDecl { name, fields, methods, generic_params, .. } => {
                    self.current_class = Some(name.clone());
                    let mut field_map = HashMap::new();
                    let mut static_field_map = HashMap::new();
                    for f in fields {
                        if let Stmt::VarDecl { name: f_name, type_annotation, is_static, .. } = f {
                            let f_ty = if let Some(ty_str) = type_annotation {
                                self.resolve_type_name(ty_str)
                            } else {
                                Type::Unknown
                            };
                            if *is_static {
                                static_field_map.insert(f_name.clone(), f_ty);
                            } else {
                                field_map.insert(f_name.clone(), f_ty);
                            }
                        }
                    }

                    let mut method_map = HashMap::new();
                    for m in methods {
                        if let Stmt::FuncDecl { name: m_name, params, return_type, visibility, is_static, .. } = m {
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
                                generic_params: generic_params.clone(),
                                is_static: *is_static,
                            };
                            method_map.insert(m_name.clone(), sig);
                        }
                    }

                    let sig = ClassSignature {
                        generic_params: generic_params.clone(),
                        fields: field_map,
                        static_fields: static_field_map,
                        methods: method_map,
                    };
                    self.env.register_class(name.clone(), sig);
                    self.current_class = None;
                }
                Stmt::ActorDecl { name, fields, methods, generic_params, .. } => {
                    self.current_class = Some(name.clone());
                    let mut field_map = HashMap::new();
                    let mut static_field_map = HashMap::new();
                    for f in fields {
                        if let Stmt::VarDecl { name: f_name, type_annotation, is_static, .. } = f {
                            let f_ty = if let Some(ty_str) = type_annotation {
                                self.resolve_type_name(ty_str)
                            } else {
                                Type::Unknown
                            };
                            if *is_static {
                                static_field_map.insert(f_name.clone(), f_ty);
                            } else {
                                field_map.insert(f_name.clone(), f_ty);
                            }
                        }
                    }

                    let mut method_map = HashMap::new();
                    for m in methods {
                        if let Stmt::FuncDecl { name: m_name, params, return_type, visibility, is_static, .. } = m {
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
                                generic_params: generic_params.clone(),
                                is_static: *is_static,
                            };
                            method_map.insert(m_name.clone(), sig);
                        }
                    }

                    let sig = crate::env::ActorSignature {
                        generic_params: generic_params.clone(),
                        fields: field_map,
                        static_fields: static_field_map,
                        methods: method_map,
                    };
                    self.env.register_actor(name.clone(), sig);
                    self.current_class = None;
                }
                Stmt::StructDecl { name, fields, generic_params, .. } => {
                    let mut field_map = HashMap::new();
                    let mut static_field_map = HashMap::new();
                    for f in fields {
                        if let Stmt::VarDecl { name: f_name, type_annotation, is_static, .. } = f {
                            let f_ty = if let Some(ty_str) = type_annotation {
                                self.resolve_type_name(ty_str)
                            } else {
                                Type::Unknown
                            };
                            if *is_static {
                                static_field_map.insert(f_name.clone(), f_ty);
                            } else {
                                field_map.insert(f_name.clone(), f_ty);
                            }
                        }
                    }
                    let sig = ClassSignature {
                        generic_params: generic_params.clone(),
                        fields: field_map,
                        static_fields: static_field_map,
                        methods: HashMap::new(),
                    };
                    self.env.register_struct(name.clone(), sig);
                }
                Stmt::EnumDecl { name, variants, generic_params, .. } => {
                    let mut variant_map = HashMap::new();
                    self.current_class = Some(name.clone());
                    
                    if let Some(params) = generic_params {
                        self.env.push_scope();
                        for param in params {
                            self.define_var(param.clone(), Type::GenericParameter(param.clone()), (0, 0), false);
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
                Stmt::InterfaceDecl { name, methods, generic_params, .. } => {
                    self.current_class = Some(name.clone());
                    let mut method_map = HashMap::new();
                    for m in methods {
                        if let Stmt::FuncDecl { name: m_name, params, return_type, visibility, is_static, .. } = m {
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
                                is_static: *is_static,
                            };
                            method_map.insert(m_name.clone(), sig);
                        }
                    }
                    let sig = ClassSignature {
                        generic_params: generic_params.clone(),
                        fields: HashMap::new(), static_fields: HashMap::new(),
                        methods: method_map,
                    };
                    self.env.register_class(name.clone(), sig);
                    self.current_class = None;
                }
                Stmt::Import { path, .. }
                    // Basic placeholder for module resolution.
                    // For now, if we import "std/collection", we mock registering `List` and `Set`
                    if path == "std/collection" => {
                        self.env.register_class("List".to_string(), ClassSignature {
                            generic_params: Some(vec!["T".to_string()]),
                            fields: HashMap::new(), static_fields: HashMap::new(),
                            methods: HashMap::new(),
                        });
                        self.env.register_class("Set".to_string(), ClassSignature {
                            generic_params: Some(vec!["T".to_string()]),
                            fields: HashMap::new(), static_fields: HashMap::new(),
                            methods: HashMap::new(),
                        });
                    }
                _ => {}
            }
        }
        ()
    }

    fn check_stmt(&mut self, stmt: &Stmt)  {
        match stmt {
            Stmt::Module { name, body } => {
                let old = self.current_module.clone();
                self.current_module = name.clone();
                for s in body {
                    self.check_stmt(s);
                }
                self.current_module = old;
            }
            Stmt::Expr(expr) => {
                self.check_expr(expr);
            }
            Stmt::VarDecl { name, is_mutable, type_annotation, initializer, span, .. } => {
                self.current_span = *span;
                let mut inferred_type = Type::Unknown;
                
                if let Some(init_expr) = initializer {
                    inferred_type = self.check_expr(init_expr);
                }
                
                if let Some(annotation) = type_annotation {
                    let expected_type = self.resolve_type_name(annotation);
                    let mut is_match = false;
                    
                    if inferred_type == expected_type || inferred_type == Type::Unknown || expected_type == Type::Any || inferred_type == Type::Any {
                        is_match = true;
                    } else if let Type::Nullable(inner) = &expected_type {
                        if inferred_type == Type::Null || inferred_type == **inner {
                            is_match = true;
                        }
                    }
                    
                    if !is_match {
                        self.define_var(name.clone(), expected_type.clone(), *span, *is_mutable);
                        { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                            message: format!(
                                "Type mismatch: expected {:?}, found {:?}",
                                expected_type, inferred_type
                            )
                        }); return (); };
                    }
                    inferred_type = expected_type;
                }
                
                if inferred_type == Type::Unknown {
                    { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                        message: format!("Cannot infer type for variable '{}'", name)
                    }); return (); };
                }
                
                if !is_camel_case(name) && !name.contains("__") {
                    self.warnings.push(pace_errors::SemanticWarning::NamingConvention {
                        name: name.clone(),
                        src: self.get_source(),
                        span: *span,
                    });
                }
                self.define_var(name.clone(), inferred_type, *span, *is_mutable);
            }
            Stmt::Block(stmts) => {
                self.env.push_scope();
                for s in stmts {
                    self.check_stmt(s);
                }
                self.pop_scope_and_check_unused();
            }
            Stmt::Return(expr_opt) => {
                let ret_ty = if let Some(expr) = expr_opt {
                    self.check_expr(expr)
                } else {
                    Type::Void
                };
                
                if let Some(expected) = &self.current_return_type {
                    let mut is_match = false;
                    
                    if expected == &ret_ty || expected == &Type::Unknown || ret_ty == Type::Unknown || expected == &Type::Any || ret_ty == Type::Any {
                        is_match = true;
                    } else if let Type::Nullable(inner) = expected {
                        if ret_ty == Type::Null || ret_ty == **inner {
                            is_match = true;
                        }
                    }
                    
                    if !is_match {
                        { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                            message: format!("Type mismatch: expected return type {:?}, found {:?}", expected, ret_ty)
                        }); return (); };
                    }
                } else {
                    { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                        message: "Return statement outside of function".to_string()
                    }); return (); };
                }
            }
            Stmt::If { condition, then_branch, else_branch } => {
                let cond_ty = self.check_expr(condition);
                if cond_ty != Type::Bool && cond_ty != Type::Unknown {
                    { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                        message: "If condition must be a boolean".to_string()
                    }); return (); };
                }
                self.check_stmt(then_branch);
                if let Some(else_b) = else_branch {
                    self.check_stmt(else_b);
                }
            }
            Stmt::While { condition, body } => {
                let cond_ty = self.check_expr(condition);
                if cond_ty != Type::Bool && cond_ty != Type::Unknown {
                    { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                        message: "While condition must be a boolean".to_string()
                    }); return (); };
                }
                self.check_stmt(body);
            }
            Stmt::Loop { body } => {
                self.check_stmt(body);
            }
            Stmt::ForIn { item, iterable, body, .. } => {
                let iterable_ty = self.check_expr(iterable);
                let mut item_ty = Type::Unknown;
                
                if let Type::GenericInstance { base: _, args } = &iterable_ty {
                    if !args.is_empty() {
                        item_ty = args[0].clone();
                    }
                } else if let Type::Class(_name) = &iterable_ty {
                    // Fallback for non-generic classes if needed, though most iterables are generic
                }
                
                self.env.push_scope();
                self.define_var(item.clone(), item_ty, (0, 0), false);
                self.check_stmt(body);
                self.pop_scope_and_check_unused();
            }
            Stmt::Match { expr, arms } => {
                let expr_ty = self.check_expr(expr);
                for (pattern, body) in arms {
                    self.env.push_scope();
                    self.check_pattern(pattern, &expr_ty);
                    self.check_stmt(body);
                    self.pop_scope_and_check_unused();
                }
            }
            Stmt::FuncDecl { name: _, params, body, return_type, generic_params, is_static, span, .. } => {
                self.current_span = *span;
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
                
                // Add `self` if we are inside a class/struct AND the method is not static
                if let Some(class_name) = &self.current_class {
                    if !is_static {
                        let self_ty = if self.env.structs.contains_key(class_name) {
                            Type::Struct(class_name.clone())
                        } else if self.env.actors.contains_key(class_name) {
                            Type::Actor(class_name.clone())
                        } else {
                            Type::Class(class_name.clone())
                        };
                        self.define_var("self".to_string(), self_ty, (0, 0), false);
                    }
                }
                
                // Add parameters to scope
                for param in params {
                    let param_type = self.resolve_type_name(&param.type_annotation);
                    self.define_var(param.name.clone(), param_type, (0, 0), false);
                }
                
                // Check body
                for s in body {
                    self.check_stmt(s);
                }
                
                self.pop_scope_and_check_unused();
                self.current_return_type = prev_return;
                self.generic_params_in_scope = prev_generics;
            }
            Stmt::ClassDecl { name, methods, implements, generic_params, .. } | Stmt::ActorDecl { name, methods, implements, generic_params, .. } => {
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
                                { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                                    message: format!("Class '{}' does not implement method '{}' from interface '{}'", name, m_name, iface_name)
                                }); return (); };
                            }
                        }
                    } else {
                        { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                            message: format!("Interface '{}' not found", iface_name)
                        }); return (); };
                    }
                }
                
                self.env.push_scope();
                for m in methods {
                    self.check_stmt(m);
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
        ()
    }

    fn check_expr(&mut self, expr: &Expr) -> Type {
        match expr {
            Expr::IntLiteral(_) => Type::Int,
            Expr::FloatLiteral(_) => Type::Float,
            Expr::StringLiteral(_) => Type::String,
            Expr::GenericInstantiation { callee, .. } => {
                self.check_expr(callee)
            }
            Expr::InterpolatedString(parts) => {
                for part in parts {
                    let ty = self.check_expr(part);
                    if ty != Type::String && ty != Type::Int && ty != Type::Float && ty != Type::Bool {
                        { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                            message: format!("Cannot interpolate value of type {:?}", ty)
                        }); return Type::Error; };
                    }
                }
                Type::String
            }
            Expr::BoolLiteral(_) => Type::Bool,
            Expr::Null => Type::Null,
            Expr::Closure { params, return_type, body } => {
                self.env.push_scope();
                
                let mut param_types = Vec::new();
                for (param_name, param_ty_ann) in params {
                    let param_ty = self.resolve_type_name(param_ty_ann);
                    param_types.push(param_ty.clone());
                    let _ = self.env.define(param_name.clone(), param_ty, (0, 0), true);
                }
                
                let ret_ty = if let Some(rt) = return_type {
                    self.resolve_type_name(rt)
                } else {
                    Type::Unknown
                };
                
                let old_expected_return = self.current_return_type.clone();
                self.current_return_type = Some(ret_ty.clone());
                
                let body_ty = self.check_expr(body);
                
                self.current_return_type = old_expected_return;
                self.pop_scope_and_check_unused();
                
                let final_ret = if ret_ty != Type::Unknown { ret_ty } else { body_ty };
                
                Type::Function {
                    params: param_types,
                    return_type: Box::new(final_ret),
                }
            }
            Expr::Block(stmts) => {
                self.env.push_scope();
                for stmt in stmts {
                    self.check_stmt(stmt);
                }
                self.pop_scope_and_check_unused();
                Type::Void
            }
            Expr::Identifier(name) => {
                if let Some(var_info) = self.env.get_mut(name) {
                    var_info.is_used = true;
                }
                match self.env.get(name) {
                    Some(ty) => ty.clone(),
                    None => {
                        // Check if it's a class/struct for instantiation
                        // Check if it's a module item
                        if self.env.classes.contains_key(name) {
                            Type::Class(name.clone())
                        } else if self.env.actors.contains_key(name) {
                            Type::Actor(name.clone())
                        } else if self.env.structs.contains_key(name) {
                            Type::Struct(name.clone())
                        } else if self.env.enums.contains_key(name) {
                            Type::Enum(name.clone())
                        } else {
                            {
                            let suggestion = self.env.find_closest_variable(&name);
                            let help_text = if let Some(sug) = suggestion {
                                format!("Did you mean '{}'?", sug)
                            } else {
                                "Variable does not exist.".to_string()
                            };
                            self.errors.push(TypeError::UnknownIdentifier {
                                name: name.clone(),
                                help_text,
                                src: self.get_source(),
                                span: self.get_span_for(&name),
                            });
                            Type::Error
                        }
                        }
                    }
                }
            }
            Expr::Binary { left, op, right } => {
                let left_ty = self.check_expr(left);
                let right_ty = self.check_expr(right);
                
                let mut types_match = left_ty == right_ty;
                if matches!(left_ty, Type::Nullable(_)) && right_ty == Type::Null {
                    types_match = true;
                }
                if matches!(right_ty, Type::Nullable(_)) && left_ty == Type::Null {
                    types_match = true;
                }
                
                if !types_match && left_ty != Type::Unknown && right_ty != Type::Unknown && left_ty != Type::Any && right_ty != Type::Any {
                    { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                        message: format!("Type mismatch in binary operation: {:?} and {:?}", left_ty, right_ty)
                    }); return Type::Error; };
                }
                
                match op {
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
                        if left_ty == Type::Int || left_ty == Type::Float || left_ty == Type::Unknown || left_ty == Type::Any || right_ty == Type::Unknown || right_ty == Type::Any {
                            left_ty
                        } else {
                            { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                                message: "Arithmetic operations require numeric types".to_string()
                            }); Type::Error }
                        }
                    }
                    BinaryOp::Eq | BinaryOp::NotEq => Type::Bool,
                    BinaryOp::Less | BinaryOp::LessEq | BinaryOp::Greater | BinaryOp::GreaterEq => {
                        if left_ty == Type::Int || left_ty == Type::Float {
                            Type::Bool
                        } else {
                            { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                                message: "Relational operations require numeric types".to_string()
                            }); Type::Error }
                        }
                    }
                    BinaryOp::And | BinaryOp::Or => {
                        if left_ty == Type::Bool {
                            Type::Bool
                        } else {
                            { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                                message: "Logical operations require boolean types".to_string()
                            }); Type::Error }
                        }
                    }
                }
            }
            Expr::Assign { target, value } => {
                let val_ty = self.check_expr(value);
                
                if let Expr::Identifier(name) = &**target {
                    let mut is_err = false;
                    let mut err_msg = String::new();
                    let mut var_span = (0, 0);
                    
                    if let Some(var_info) = self.env.get_mut(name) {
                        if !var_info.is_mutable {
                            is_err = true;
                            err_msg = format!("Cannot assign to immutable variable '{}'", name);
                            var_span = var_info.span;
                        } else if var_info.ty != val_ty && var_info.ty != Type::Unknown && val_ty != Type::Unknown && var_info.ty != Type::Any && val_ty != Type::Any {
                            is_err = true;
                            err_msg = format!("Type mismatch: cannot assign {:?} to variable of type {:?}", val_ty, var_info.ty);
                            var_span = var_info.span;
                        } else {
                            var_info.is_used = true;
                        }
                    } else {
                        let suggestion = self.env.find_closest_variable(&name);
                        let help_text = if let Some(sug) = suggestion {
                            format!("Did you mean '{}'?", sug)
                        } else {
                            "Variable does not exist.".to_string()
                        };
                        self.errors.push(TypeError::UnknownIdentifier {
                            name: name.clone(),
                            help_text,
                            src: self.get_source(),
                            span: self.get_span_for(&name),
                        });
                        return Type::Error;
                    }
                    
                    if is_err {
                        self.errors.push(TypeError::Generic { src: self.get_source(), span: var_span, message: err_msg });
                        return Type::Error;
                    }
                    val_ty
                } else if let Expr::MemberAccess { object, property: _, computed_class: _, is_static_operator: _ } = &**target {
                    let _obj_ty = self.check_expr(object);
                    // Simple validation for now - real validation needs class layout check
                    val_ty
                } else {
                    { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                        message: "Invalid assignment target".to_string()
                    }); Type::Error }
                }
            }
            Expr::Call { callee, args } => {
                let callee_ty = self.check_expr(callee);
                
                let mut arg_types = Vec::new();
                for arg in args {
                    arg_types.push(self.check_expr(arg));
                }
                
                // If callee is a known class/struct, it's a constructor call
                if let Type::Class(name) = &callee_ty {
                    if let Some(_sig) = self.env.classes.get(name) {
                        return Type::Class(name.clone());
                    }
                } else if let Type::Actor(name) = &callee_ty
                    && let Some(_sig) = self.env.actors.get(name) {
                        return Type::Actor(name.clone());
                } else if let Type::Struct(name) = &callee_ty
                    && let Some(_sig) = self.env.structs.get(name) {
                        return Type::Struct(name.clone());
                } else if let Type::Enum(name) = &callee_ty
                    && let Some(_sig) = self.env.enums.get(name) {
                        return Type::Enum(name.clone());
                }
                
                // If it's a function or method, we need its signature
                // Currently, callee_ty might just be Type::Unknown if it was a MemberAccess
                // So if we don't know the type, we just return Unknown.
                
                // For first-class function values (closures, callbacks)
                if let Type::Function { params, return_type } = &callee_ty {
                    if params.len() != args.len() {
                        { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                            message: format!("Function expects {} arguments, got {}", params.len(), args.len())
                        }); return Type::Error; };
                    }
                    
                    for (i, arg_ty) in arg_types.iter().enumerate() {
                        let expected_ty = &params[i];
                        if expected_ty != &Type::Any && expected_ty != arg_ty && arg_ty != &Type::Unknown {
                            { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                                message: format!("Type mismatch in argument {}: expected {:?}, got {:?}", i + 1, expected_ty, arg_ty)
                            }); return Type::Error; };
                        }
                    }
                    return (**return_type).clone();
                }

                // For direct global function calls
                if let Expr::Identifier(func_name) = &**callee
                    && let Some(sig) = self.env.functions.get(func_name) {
                        if sig.visibility == Visibility::Private && sig.module != self.current_module {
                            { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                                message: format!("Function '{}' is private and cannot be accessed outside of module '{}'", func_name, sig.module)
                            }); return Type::Error; };
                        }
                        if sig.params.len() != args.len() {
                            { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                                message: format!("Function '{}' expects {} arguments, got {}", func_name, sig.params.len(), args.len())
                            }); return Type::Error; };
                        }
                        
                        for (i, arg_ty) in arg_types.iter().enumerate() {
                            let expected_ty = &sig.params[i];
                            if expected_ty != &Type::Any && expected_ty != arg_ty {
                                { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                                    message: format!("Type mismatch in argument {}: expected {:?}, got {:?}", i + 1, expected_ty, arg_ty)
                                }); return Type::Error; };
                            }
                        }
                        return sig.return_type.clone();
                    }
                
                // For member access calls (e.g. self.client.get())
                // MemberAccess returns the method's return type, so we just return callee_ty
                callee_ty
            }
            Expr::MemberAccess { object, property, computed_class: _, is_static_operator } => {
                let mut is_namespace_access = false;
                let mut base_ident = None;
                if let Expr::Identifier(name) = &**object {
                    base_ident = Some(name.clone());
                } else if let Expr::GenericInstantiation { callee, .. } = &**object {
                    if let Expr::Identifier(name) = &**callee {
                        base_ident = Some(name.clone());
                    }
                }
                if let Some(ref name) = base_ident {
                    if self.env.classes.contains_key(name) || self.env.structs.contains_key(name) || self.env.enums.contains_key(name) || self.env.actors.contains_key(name) {
                        is_namespace_access = !self.env.is_local(name);
                    }
                }
                
                // Allow :: ONLY on namespaces, and . ONLY on instances.
                // Exception: allow . on namespaces ONLY if it's NOT an enum variant (for backwards compatibility while we transition)
                if *is_static_operator && !is_namespace_access {
                    { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                        message: format!("The '::' operator can only be used for static or namespace access (object was {:?}, base_ident {:?}, classes={:?})", object, base_ident, self.env.classes.keys())
                    }); return Type::Error; };
                }
                if !*is_static_operator && is_namespace_access {
                    { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                        message: "The '.' operator can only be used for instance access. Use '::' for static/namespace access.".to_string()
                    }); return Type::Error; };
                }
                
                let obj_ty = self.check_expr(object);
                
                let (class_name, fields, static_fields, methods) = match obj_ty {
                    Type::Class(ref name) => {
                        let sig = match self.env.classes.get(name) {
                            Some(s) => s,
                            None => {
                                self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                                    message: format!("Type '{}' is not defined", name)
                                });
                                return Type::Error;
                            }
                        };
                        (name.clone(), sig.fields.clone(), sig.static_fields.clone(), sig.methods.clone())
                    },
                    Type::Actor(ref name) => {
                        let sig = match self.env.actors.get(name) {
                            Some(s) => s,
                            None => {
                                self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                                    message: format!("Actor '{}' is not defined", name)
                                });
                                return Type::Error;
                            }
                        };
                        (name.clone(), sig.fields.clone(), sig.static_fields.clone(), sig.methods.clone())
                    },
                    Type::Struct(ref name) => {
                        let sig = match self.env.structs.get(name) {
                            Some(s) => s,
                            None => {
                                self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                                    message: format!("Type '{}' is not defined", name)
                                });
                                return Type::Error;
                            }
                        };
                        (name.clone(), sig.fields.clone(), sig.static_fields.clone(), sig.methods.clone())
                    },
                    Type::Enum(ref name) => {
                        let sig = match self.env.enums.get(name) {
                            Some(s) => s,
                            None => {
                                self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                                    message: format!("Enum '{}' is not defined", name)
                                });
                                return Type::Error;
                            }
                        };
                        if sig.variants.contains_key(property) {
                            return Type::Enum(name.clone());
                        }
                        { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                            message: format!("Enum '{}' has no variant '{}'", name, property)
                        }); return Type::Error; };
                    },
                    _ => {
                        { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                            message: format!("Cannot access property '{}' on non-object type", property)
                        }); return Type::Error; };
                    }
                };
                
                if let Some(ty) = static_fields.get(property) {
                    return ty.clone();
                }
                if let Some(ty) = fields.get(property) {
                    if let Type::Actor(ref a_name) = obj_ty {
                        if Some(a_name.clone()) != self.current_class {
                            { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                                message: format!("Actor fields are isolated and cannot be accessed from outside actor '{}'", a_name.split("__").last().unwrap_or(a_name))
                            }); return Type::Error; };
                        }
                    }
                    return ty.clone();
                }
                if let Some(m_sig) = methods.get(property) {
                    if m_sig.visibility == Visibility::Private {
                        if self.current_class.as_deref() != Some(&*class_name) {
                            { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                                message: format!("Method '{}' is private and cannot be accessed from outside class/actor '{}'", property, class_name.split("__").last().unwrap_or(&class_name))
                            }); return Type::Error; };
                        }
                    }
                    if matches!(obj_ty, Type::Actor(_)) {
                        return Type::Promise(Box::new(m_sig.return_type.clone()));
                    }
                    return m_sig.return_type.clone();
                }
                { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                    message: format!("Property '{}' not found on type '{}'", property, &class_name)
                }); Type::Error }
            }
            Expr::Await(inner) => {
                let inner_ty = self.check_expr(inner);
                if let Type::Promise(t) = inner_ty {
                    *t
                } else {
                    { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span, message: "Cannot await a non-promise type".to_string() }); Type::Error }
                }
            }
            Expr::Unwrap(inner) => {
                let inner_ty = self.check_expr(inner);
                if let Type::Nullable(t) = inner_ty {
                    *t
                } else {
                    { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span, message: "Cannot unwrap a non-nullable type".to_string() }); Type::Error }
                }
            }
            Expr::Try(inner) => {
                let inner_ty = self.check_expr(inner);
                if let Type::Enum(name) = &inner_ty {
                    if let Some(sig) = self.env.enums.get(name) {
                        if name.starts_with("Result_") {
                            if let Some(Type::Enum(ret_name)) = &self.current_return_type {
                                if !ret_name.starts_with("Result_") {
                                    { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span, message: "Cannot use ? on a Result in a function that does not return Result".to_string() }); return Type::Error; };
                                }
                            } else {
                                { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span, message: "Cannot use ? on a Result in a function that does not return Result".to_string() }); return Type::Error; };
                            }
                            if let Some(Some(fields)) = sig.variants.get("Ok") {
                                if let Some(t) = fields.first() {
                                    return t.clone();
                                }
                            }
                            return Type::Void;
                        } else if name.starts_with("Option_") {
                            if let Some(Type::Enum(ret_name)) = &self.current_return_type {
                                if !ret_name.starts_with("Option_") {
                                    { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span, message: "Cannot use ? on an Option in a function that does not return Option".to_string() }); return Type::Error; };
                                }
                            } else {
                                { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span, message: "Cannot use ? on an Option in a function that does not return Option".to_string() }); return Type::Error; };
                            }
                            if let Some(Some(fields)) = sig.variants.get("Some") {
                                if let Some(t) = fields.first() {
                                    return t.clone();
                                }
                            }
                            return Type::Void;
                        }
                    }
                }
                { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span, message: "The ? operator can only be applied to Result or Option types".to_string() }); Type::Error }
            }
            Expr::NullCoalesce { left, right } => {
                let left_ty = self.check_expr(left);
                let right_ty = self.check_expr(right);
                if let Type::Nullable(inner) = left_ty {
                    if *inner == right_ty {
                        *inner
                    } else if right_ty == Type::Null {
                        Type::Nullable(inner)
                    } else {
                        { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span, message: format!("Null coalesce type mismatch: {:?} and {:?}", *inner, right_ty) }); Type::Error }
                    }
                } else {
                    { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span, message: "Left side of ?? must be nullable".to_string() }); Type::Error }
                }
            }
            Expr::OptionalMemberAccess { object, property } => {
                let obj_ty = self.check_expr(object);
                if let Type::Nullable(inner) = obj_ty {
                    // Check property on inner type
                    let _inner_expr = Expr::MemberAccess {
                        object: Box::new(Expr::Null), // Dummy object to bypass recursive check_expr if we extracted logic
                        property: property.clone(),
                        computed_class: None,
                        is_static_operator: false,
                    };
                    // Instead of full check, we can manually check if it's Class or Struct
                    let (class_name, sig) = match &*inner {
                        Type::Class(name) => (name, self.env.classes.get(name).unwrap()),
                        Type::Struct(name) => (name, self.env.structs.get(name).unwrap()),
                        _ => { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span, message: "Optional access on non-object".to_string() }); return Type::Error; },
                    };
                    
                    if let Some(f_ty) = sig.fields.get(property) {
                        return Type::Nullable(Box::new(f_ty.clone()));
                    }
                    if let Some(m_sig) = sig.methods.get(property) {
                        return Type::Nullable(Box::new(m_sig.return_type.clone()));
                    }
                    { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span, message: format!("Property '{}' not found on type '{}'", property, &class_name) }); Type::Error }
                } else {
                    { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span, message: "Optional member access on non-nullable type".to_string() }); Type::Error }
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
                    } else if self.env.actors.contains_key(current) {
                        Type::Actor(current.clone())
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
                } else if self.env.actors.contains_key(base_name) {
                    Type::Actor(base_name.to_string())
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

        if annotation.is_function {
            let mut params = Vec::new();
            if let Some(fn_params) = &annotation.function_params {
                for p in fn_params {
                    params.push(self.resolve_type_name(p));
                }
            }
            let return_type = if let Some(ret) = &annotation.function_return {
                Box::new(self.resolve_type_name(ret))
            } else {
                Box::new(Type::Void)
            };
            base_type = Type::Function { params, return_type };
        }

        if annotation.is_nullable {
            Type::Nullable(Box::new(base_type))
        } else {
            base_type
        }
    }
    pub fn check_pattern(&mut self, pattern: &pace_ast::Pattern, expected_type: &Type)  {
        match pattern {
            pace_ast::Pattern::Wildcard => (),
            pace_ast::Pattern::Literal(expr) => {
                let ty = self.check_expr(expr);
                if expected_type != &ty && expected_type != &Type::Unknown && ty != Type::Unknown {
                    { self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span, message: format!("Pattern type mismatch: expected {:?}, got {:?}", expected_type, ty) }); return (); };
                }
                ()
            },
            pace_ast::Pattern::Variable(name, span) => {
                self.define_var(name.clone(), expected_type.clone(), *span, false);
                ()
            },
            pace_ast::Pattern::Variant { enum_name, variant_name, fields, generic_args: _ } => {
                let mut field_types = vec![Type::Unknown; fields.as_ref().map_or(0, |f| f.len())];
                if let Some(ename) = enum_name {
                    if let Some(sig) = self.env.enums.get(ename) {
                        if let Some((_, v_fields_opt)) = sig.variants.iter().find(|(name, _)| **name == *variant_name) {
                            if let Some(v_fields) = v_fields_opt {
                                for (i, f_ty) in v_fields.iter().enumerate() {
                                    if i < field_types.len() {
                                        field_types[i] = f_ty.clone();
                                    }
                                }
                            }
                        }
                    }
                }
                
                if let Some(fields) = fields {
                    for (i, field) in fields.iter().enumerate() {
                        self.check_pattern(field, &field_types[i]);
                    }
                }
                ()
            }
        }
    }
}

fn is_camel_case(s: &str) -> bool {
    if s.is_empty() { return true; }
    let first = s.chars().next().unwrap();
    if first.is_uppercase() { return false; }
    !s.contains('_')
}
