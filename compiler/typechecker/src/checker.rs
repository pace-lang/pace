use module::graph::ModuleGraph;
use ast::{Expr, ExprKind, Stmt, StmtKind, Span, BinaryOp, UnaryOp, TypeExpr, TypedExpr, TypedExprKind, TypedStmt, TypedStmtKind};
use ast::types::Type;
use crate::env::TypeEnvironment;
use std::collections::HashMap;
use diagnostics::{Diagnostic, DiagnosticBuilder, DiagnosticCode};

#[derive(Debug)]


pub struct TypeChecker {
    env: TypeEnvironment,
    pub errors: Vec<Diagnostic>,
    current_return_type: Option<Type>,
    pub classes: HashMap<String, HashMap<String, Type>>,
    pub interfaces: HashMap<String, HashMap<String, Type>>,
    pub enums: HashMap<String, HashMap<String, Type>>,
    pub class_implements: HashMap<String, Vec<String>>,
    current_class: Option<String>,
    pub generic_registry: generics::GenericDefinitionRegistry,
    pub spec_registry: generics::SpecializationRegistry,
    pub pending_instantiations: Vec<TypedStmt>,
    pub uninitialized_class_properties: HashMap<String, Vec<String>>,
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            env: TypeEnvironment::new(),
            errors: Vec::new(),
            current_return_type: None,
            classes: HashMap::new(),
            interfaces: HashMap::new(),
            enums: HashMap::new(),
            class_implements: HashMap::new(),
            current_class: None,
            generic_registry: generics::GenericDefinitionRegistry::new(),
            spec_registry: generics::SpecializationRegistry::new(),
            pending_instantiations: Vec::new(),
            uninitialized_class_properties: HashMap::new(),
        }
    }


    pub fn check_graph(&mut self, graph: &ModuleGraph) -> Vec<TypedStmt> {
        let mut all_stmts = Vec::new();
        for module in graph.topological_sort() {
            let typed_ast = self.check(&module.ast);
            all_stmts.extend(typed_ast);
        }
        
        // Also drain pending generic instantiations
        let mut final_stmts = Vec::new();
        while !self.pending_instantiations.is_empty() {
            let pending: Vec<TypedStmt> = self.pending_instantiations.drain(..).collect();
            final_stmts.extend(pending);
        }
        
        final_stmts.extend(all_stmts);
        final_stmts
    }

    pub fn check_program(&mut self, statements: &[Stmt]) -> Vec<TypedStmt> {
        let typed_stmts = self.check(statements);
        
        let mut final_stmts = Vec::new();
        while !self.pending_instantiations.is_empty() {
            let pending: Vec<TypedStmt> = self.pending_instantiations.drain(..).collect();
            final_stmts.extend(pending);
        }
        
        final_stmts.extend(typed_stmts);
        final_stmts
    }

    pub fn check(&mut self, statements: &[Stmt]) -> Vec<TypedStmt> {
        let mut typed_stmts = Vec::new();
        for stmt in statements {
            typed_stmts.push(self.check_stmt(stmt));
        }
        typed_stmts
    }

    fn check_stmt(&mut self, stmt: &Stmt) -> TypedStmt {
        let kind = match &stmt.kind {
            StmtKind::Block(stmts) => {
                self.env.push_scope();
                let typed_stmts = self.check(stmts);
                self.env.pop_scope();
                TypedStmtKind::Block(typed_stmts)
            }
            StmtKind::Let { name, type_annotation, initializer, is_private: _ } => self.check_var_decl(name, type_annotation, initializer, false, stmt.span).kind,
            StmtKind::Var { name, type_annotation, initializer, is_weak, is_private: _ } => self.check_var_decl(name, type_annotation, initializer, *is_weak, stmt.span).kind,
            StmtKind::Class { name, type_params, implements, methods, fields, is_private: _ } => {
                if !type_params.is_empty() {
                    self.generic_registry.register_class(name.clone(), stmt.clone());
                    return TypedStmt { kind: TypedStmtKind::Block(Vec::new()), span: stmt.span };
                }

                self.env.declare(name.clone(), Type::Class(name.clone(), type_params.clone()));
                self.classes.insert(name.clone(), std::collections::HashMap::new());
                
                self.env.push_scope();
                for tp in type_params {
                    self.env.declare(tp.clone(), Type::Generic(tp.clone()));
                }

                let mut class_members = HashMap::new();
                let mut uninit_props = Vec::new();
                
                for field in fields {
                    let (f_name, type_annotation, initializer, is_weak) = match &field.kind {
                        StmtKind::Var { name, type_annotation, initializer, is_weak, is_private: _ } => (name, type_annotation, initializer, *is_weak),
                        StmtKind::Let { name, type_annotation, initializer, is_private: _ } => (name, type_annotation, initializer, false),
                        _ => continue,
                    };
                    
                    if initializer.is_none() {
                        uninit_props.push(f_name.clone());
                    }
                    
                    let ty = if let Some(ann) = type_annotation {
                        let parsed = self.parse_type(ann, field.span);
                        if is_weak {
                            if !matches!(parsed, Type::Optional(ref inner) if matches!(**inner, Type::Instance(_) | Type::Interface(_))) {
                                self.error(field.span, DiagnosticCode::TypeMismatch, "Weak properties must be of optional instance type (e.g. 'weak var x: User?').");
                            }
                        }
                        parsed
                    } else if let Some(init) = initializer {
                        let parsed = self.check_expr(init);
                        if is_weak {
                            if !matches!(parsed.ty, Type::Optional(ref inner) if matches!(**inner, Type::Instance(_) | Type::Interface(_))) {
                                self.error(field.span, DiagnosticCode::TypeMismatch, "Weak properties must be of optional instance type (e.g. 'weak var x: User?').");
                            }
                        }
                        parsed.ty.clone()
                    } else {
                        if is_weak {
                            self.error(field.span, DiagnosticCode::TypeMismatch, "Weak properties must be of optional instance type (e.g. 'weak var x: User?').");
                        }
                        Type::Any
                    };
                    class_members.insert(f_name.clone(), ty);
                }

                self.uninitialized_class_properties.insert(name.clone(), uninit_props);

                for method in methods {
                    if let StmtKind::Func { name: m_name, params, return_type, .. } = &method.kind {
                        let ret_ty = if let Some(rt) = return_type {
                            self.parse_type(rt, method.span)
                        } else {
                            Type::Void
                        };
                        let mut param_types = Vec::new();
                        for (_, pt) in params {
                            param_types.push(self.parse_type(pt, method.span));
                        }
                        class_members.insert(m_name.clone(), Type::Function(Vec::new(), param_types, Box::new(ret_ty)));
                    }
                }
                
                self.classes.insert(name.clone(), class_members.clone());
                self.class_implements.insert(name.clone(), implements.clone());
                
                // Validate implements
                for interface_name in implements {
                    if let Some(interface_members) = self.interfaces.get(interface_name).cloned() {
                        for (i_method_name, i_method_ty) in interface_members {
                            if let Some(c_method_ty) = class_members.get(&i_method_name) {
                                if *c_method_ty != i_method_ty {
                                    self.error(stmt.span, DiagnosticCode::TypeMismatch, &format!("Class '{}' incorrectly implements method '{}' of interface '{}'. Expected '{}', found '{}'.", name, i_method_name, interface_name, i_method_ty, c_method_ty))
                                }
                            } else {
                                self.error(stmt.span, DiagnosticCode::TypeMismatch, &format!("Class '{}' does not implement required method '{}' of interface '{}'.", name, i_method_name, interface_name))
                            }
                        }
                    } else {
                        self.error(stmt.span, DiagnosticCode::UnknownType, &format!("Interface '{}' not found.", interface_name))
                    }
                }
                
                let prev_class = self.current_class.clone();
                self.current_class = Some(name.clone());
                
                let mut typed_methods = Vec::new();
                for method in methods {
                    typed_methods.push(self.check_stmt(method));
                }
                
                let mut typed_fields = Vec::new();
                for field in fields {
                    typed_fields.push(self.check_stmt(field));
                }
                
                self.env.pop_scope();
                self.current_class = prev_class;
                TypedStmtKind::Class {
                    name: name.clone(),
                    type_params: type_params.clone(),
                    implements: implements.clone(),
                    methods: typed_methods,
                    fields: typed_fields,
                }
            }
            StmtKind::Interface { name, methods, is_private: _ } => {
                let mut interface_methods = std::collections::HashMap::new();
                for method in methods {
                    if let StmtKind::Func { name: m_name, params, return_type, .. } = &method.kind {
                        let ret_ty = if let Some(rt) = return_type {
                            self.parse_type(rt, method.span)
                        } else {
                            Type::Void
                        };
                        let mut param_types = Vec::new();
                        for (_, pt) in params {
                            param_types.push(self.parse_type(pt, method.span));
                        }
                        interface_methods.insert(m_name.clone(), Type::Function(Vec::new(), param_types, Box::new(ret_ty)));
                    }
                }
                
                self.interfaces.insert(name.clone(), interface_methods);
                self.env.declare(name.clone(), Type::Interface(name.clone()));
                
                TypedStmtKind::Interface {
                    name: name.clone(),
                    methods: Vec::new(),
                }
            }
            StmtKind::Enum { name, type_params, variants, is_private: _ } => {
                if !type_params.is_empty() {
                    // Similar to generic classes, register them for instantiation
                    // self.generic_registry.register_enum(name.clone(), stmt.clone());
                }

                self.env.declare(name.clone(), Type::Enum(name.clone(), type_params.clone()));
                
                self.env.push_scope();
                for tp in type_params {
                    self.env.declare(tp.clone(), Type::Generic(tp.clone()));
                }
                
                let mut enum_variants = HashMap::new();
                
                for variant in variants {
                    let mut param_types = Vec::new();
                    if let Some(fields) = &variant.fields {
                        for field in fields {
                            param_types.push(self.parse_type(&field.ty, stmt.span));
                        }
                    }
                    
                    let ret_ty = if type_params.is_empty() {
                        Type::Instance(name.clone())
                    } else {
                        let mut ret_args = Vec::new();
                        for tp in type_params {
                            ret_args.push(Type::Generic(tp.clone()));
                        }
                        Type::GenericInstance(name.clone(), ret_args)
                    };
                    
                    let variant_ty = Type::EnumVariantConstructor(name.clone(), variant.name.clone(), type_params.clone(), param_types, Box::new(ret_ty));
                    
                    enum_variants.insert(variant.name.clone(), variant_ty);
                }
                
                self.env.pop_scope();
                
                // Declare variants in the enclosing scope
                for (variant_name, variant_ty) in &enum_variants {
                    self.env.declare(variant_name.clone(), variant_ty.clone());
                }
                
                self.enums.insert(name.clone(), enum_variants);

                TypedStmtKind::Enum {
                    name: name.clone(),
                    type_params: type_params.clone(),
                    variants: variants.clone(),
                }
            }
            StmtKind::ForeignFunc { name, type_params, params, return_type, is_private: _ } => {
                self.env.push_scope();
                for tp in type_params {
                    self.env.declare(tp.clone(), Type::Generic(tp.clone()));
                }

                let ret_ty = if let Some(rt) = return_type {
                    self.parse_type(rt, stmt.span)
                } else {
                    Type::Void
                };

                let mut param_types = Vec::new();
                for (_, param_type_str) in params {
                    param_types.push(self.parse_type(param_type_str, stmt.span));
                }

                self.env.pop_scope();
                
                self.env.declare(name.clone(), Type::Function(type_params.clone(), param_types.clone(), Box::new(ret_ty.clone())));
                TypedStmtKind::ForeignFunc {
                    name: name.clone(),
                    params: params.clone(),
                    return_type: return_type.clone(),
                }
            }
            StmtKind::Func { name, type_params, params, return_type, body, is_private: _ } => {
                if !type_params.is_empty() {
                    self.generic_registry.register_function(name.clone(), stmt.clone());
                    return TypedStmt { kind: TypedStmtKind::Block(Vec::new()), span: stmt.span };
                }

                // Parse return type from AST string
                let ret_ty = if let Some(rt) = return_type {
                    self.parse_type(rt, stmt.span)
                } else {
                    Type::Void
                };

                let mut param_types = Vec::new();
                for (_, param_type_str) in params {
                    param_types.push(self.parse_type(param_type_str, stmt.span));
                }

                self.env.declare(name.clone(), Type::Function(type_params.clone(), param_types.clone(), Box::new(ret_ty.clone())));

                self.env.push_scope();
                for tp in type_params {
                    self.env.declare(tp.clone(), Type::Generic(tp.clone()));
                }

                
                if let Some(ref class_name) = self.current_class {
                    self.env.declare("self".to_string(), Type::Instance(class_name.clone()));
                }

                for ((param_name, _), param_ty) in params.iter().zip(param_types.into_iter()) {
                    self.env.declare(param_name.clone(), param_ty);
                }

                let previous_return = self.current_return_type.take();
                self.current_return_type = Some(ret_ty.clone());

                let typed_body = self.check_stmt(body);

                if name == "init" {
                    if let Some(ref class_name) = self.current_class {
                        if let Some(uninit_props_ref) = self.uninitialized_class_properties.get(class_name) {
                            let uninit_props = uninit_props_ref.clone();
                            let assigned_props = Self::get_assigned_properties_in_init(&typed_body);
                            for prop in uninit_props {
                                if !assigned_props.contains(&prop) {
                                    self.error(stmt.span, DiagnosticCode::UninitializedVariable, &format!("Property '{}' is not initialized by the constructor.", prop));
                                }
                            }
                        }
                    }
                }

                self.current_return_type = previous_return;
                self.env.pop_scope();
                TypedStmtKind::Func {
                    name: name.clone(),
                    type_params: type_params.clone(),
                    params: params.clone(),
                    return_type: return_type.clone(),
                    body: Box::new(typed_body),
                }
            }
            StmtKind::If { condition, then_branch, else_branch } => {
                let typed_condition = self.check_expr(condition);
                if typed_condition.ty != Type::Boolean && typed_condition.ty != Type::Error {
                    self.error(condition.span, DiagnosticCode::TypeMismatch, &format!("Expected 'Boolean' for if condition, found '{}'.", typed_condition.ty))
                }

                let typed_then = self.check_stmt(then_branch);
                let typed_else = if let Some(e_branch) = else_branch {
                    Some(Box::new(self.check_stmt(e_branch)))
                } else {
                    None
                };
                TypedStmtKind::If {
                    condition: typed_condition,
                    then_branch: Box::new(typed_then),
                    else_branch: typed_else,
                }
            }
            StmtKind::While { condition, body } => {
                let typed_condition = self.check_expr(condition);
                if typed_condition.ty != Type::Boolean && typed_condition.ty != Type::Error {
                    self.error(condition.span, DiagnosticCode::TypeMismatch, &format!("Expected 'Boolean' for while condition, found '{}'.", typed_condition.ty))
                }

                let typed_body = self.check_stmt(body);
                TypedStmtKind::While {
                    condition: typed_condition,
                    body: Box::new(typed_body),
                }
            }
            StmtKind::For { item_name, iterator, body } => {
                let typed_iterator = self.check_expr(iterator);
                // Basic implementation: we don't know what type is inside the iterator yet without generics/arrays.
                // We will default the item to Error so it ignores subsequent type errors inside the loop.
                self.env.push_scope();
                self.env.declare(item_name.clone(), Type::Error);
                let typed_body = self.check_stmt(body);
                self.env.pop_scope();
                TypedStmtKind::For {
                    item_name: item_name.clone(),
                    iterator: typed_iterator,
                    body: Box::new(typed_body),
                }
            }
            StmtKind::Import { .. } | StmtKind::Export { .. } => ast::TypedStmtKind::Block(vec![]),
            StmtKind::Expression(expr) => {
                TypedStmtKind::Expression(self.check_expr(expr))
            }
            StmtKind::Return { value } => {
                let typed_val = if let Some(val) = value {
                    let expected_ret = self.current_return_type.clone();
                    let mut tv = self.check_expr_with_expected(val, expected_ret.as_ref());
                    
                    if let Some(expected_ty) = &self.current_return_type {
                        if !self.is_assignable(&tv.ty, expected_ty) && tv.ty != Type::Error && *expected_ty != Type::Error && *expected_ty != Type::Any {
                            self.error(stmt.span, DiagnosticCode::TypeMismatch, &format!("Expected return type '{}', found '{}'.", expected_ty, tv.ty));
                            tv.ty = Type::Error;
                        }
                    } else {
                        self.error(stmt.span, DiagnosticCode::TypeMismatch, "Cannot return from outside a function.");
                    }
                    Some(tv)
                } else {
                    if self.current_return_type.is_some() && *self.current_return_type.as_ref().unwrap() != Type::Void {
                        self.error(stmt.span, DiagnosticCode::TypeMismatch, &format!("Expected return type '{}', found 'Void'.", self.current_return_type.as_ref().unwrap()));
                    }
                    None
                };
                TypedStmtKind::Return { value: typed_val }
            }
        };
        TypedStmt::new(kind, stmt.span)
    }

    fn check_var_decl(&mut self, name: &String, type_annotation: &Option<TypeExpr>, initializer: &Option<Expr>, is_weak: bool, span: Span) -> TypedStmt {
        let expected_ty = type_annotation.as_ref().map(|ann| self.parse_type(ann, span));
        
        let typed_init = if let Some(init) = initializer {
            Some(self.check_expr_with_expected(init, expected_ty.as_ref()))
        } else {
            None
        };
        
        let mut init_type = typed_init.as_ref().map(|t| &t.ty).unwrap_or(&Type::Any).clone();
        
        if let Some(ann_type) = expected_ty {
            if init_type == Type::Any {
                init_type = ann_type.clone();
            } else if !self.is_assignable(&init_type, &ann_type) && init_type != Type::Error {
                self.error(span, DiagnosticCode::TypeMismatch, &format!("Cannot assign type '{}' to variable of type '{}'.", init_type, ann_type));
            }
            
            if is_weak {
                if !matches!(ann_type, Type::Optional(ref inner) if matches!(**inner, Type::Instance(_) | Type::Interface(_))) {
                    self.error(span, DiagnosticCode::TypeMismatch, "Weak variables must be of optional instance type (e.g. 'weak var x: User?').");
                }
            }
        } else if is_weak {
             if !matches!(init_type, Type::Optional(ref inner) if matches!(**inner, Type::Instance(_) | Type::Interface(_))) {
                 self.error(span, DiagnosticCode::TypeMismatch, "Weak variables must be of optional instance type (e.g. 'weak var x: User?').");
             }
        }
        
        self.env.declare(name.clone(), init_type);
        
        let kind = if is_weak || (initializer.is_none() && type_annotation.is_some()) {
            TypedStmtKind::Var {
                name: name.clone(),
                type_annotation: type_annotation.clone(),
                initializer: typed_init,
                is_weak,
            }
        } else {
            TypedStmtKind::Let {
                name: name.clone(),
                type_annotation: type_annotation.clone(),
                initializer: typed_init,
            }
        };
        TypedStmt::new(kind, span)
    }

    fn check_expr(&mut self, expr: &Expr) -> TypedExpr {
        self.check_expr_with_expected(expr, None)
    }

    fn check_expr_with_expected(&mut self, expr: &Expr, expected_ty: Option<&Type>) -> TypedExpr {
        let (kind, ty) = match &expr.kind {
            ExprKind::Integer(v) => (TypedExprKind::Integer(*v), Type::Int),
            ExprKind::Float(v) => (TypedExprKind::Float(*v), Type::Float),
            ExprKind::String(v) => (TypedExprKind::String(v.clone()), Type::String),
            ExprKind::Boolean(v) => (TypedExprKind::Boolean(*v), Type::Boolean),
            ExprKind::Null => (TypedExprKind::Null, Type::Null),
            ExprKind::Variable(name) => {
                if let Some(ty) = self.env.resolve(name) {
                    (TypedExprKind::Variable(name.clone()), ty)
                } else if let Some(generic_stmt) = self.generic_registry.get_class(name).cloned() {
                    if let ast::StmtKind::Class { type_params, .. } = &generic_stmt.kind {
                        (TypedExprKind::Variable(name.clone()), Type::Class(name.clone(), type_params.clone()))
                    } else {
                        unreachable!()
                    }
                } else if let Some(generic_stmt) = self.generic_registry.get_function(name).cloned() {
                    if let ast::StmtKind::Func { type_params, params, return_type, .. } = &generic_stmt.kind {
                        self.env.push_scope();
                        for tp in type_params {
                            self.env.declare(tp.clone(), Type::Generic(tp.clone()));
                        }
                        
                        let mut param_types = Vec::new();
                        for (_, ty) in params {
                            param_types.push(self.parse_type(ty, expr.span));
                        }
                        let ret_ty = if let Some(ty) = return_type {
                            self.parse_type(ty, expr.span)
                        } else {
                            Type::Void
                        };
                        
                        self.env.pop_scope();
                        
                        (TypedExprKind::Variable(name.clone()), Type::Function(type_params.clone(), param_types, Box::new(ret_ty)))
                    } else {
                        unreachable!()
                    }
                } else {
                    self.error(expr.span, DiagnosticCode::UnknownIdentifier, &format!("Variable '{}' not found.", name));
                    (TypedExprKind::Variable(name.clone()), Type::Error)
                }
            }
            ExprKind::Assign { name, value } => {
                let typed_val = self.check_expr(value);
                if let Some(var_type) = self.env.resolve(name) {
                    if typed_val.ty != var_type && typed_val.ty != Type::Error && var_type != Type::Error && var_type != Type::Any {
                        self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot assign type '{}' to variable of type '{}'.", typed_val.ty, var_type))
                    }
                } else {
                    self.error(expr.span, DiagnosticCode::UnknownIdentifier, &format!("Variable '{}' not found.", name))
                }
                (TypedExprKind::Assign { name: name.clone(), value: Box::new(typed_val.clone()) }, typed_val.ty)
            }
            ExprKind::SelfRef => {
                if let Some(ty) = self.env.resolve(&"self".to_string()) {
                    (TypedExprKind::SelfRef, ty)
                } else {
                    self.error(expr.span, DiagnosticCode::TypeMismatch, "Cannot use 'self' outside a class.");
                    (TypedExprKind::SelfRef, Type::Error)
                }
            }
            ExprKind::ForceUnwrap(inner) => {
                let typed_inner = self.check_expr(inner);
                let ty = match &typed_inner.ty {
                    Type::Optional(inner_inner) => (**inner_inner).clone(),
                    Type::Null => {
                        self.error(expr.span, DiagnosticCode::TypeMismatch, "Cannot force unwrap a null literal.");
                        Type::Error
                    },
                    Type::Error | Type::Any => typed_inner.ty.clone(),
                    _ => {
                        self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot force unwrap non-optional type '{}'.", typed_inner.ty));
                        typed_inner.ty.clone()
                    }
                };
                (TypedExprKind::ForceUnwrap(Box::new(typed_inner)), ty)
            }
            ExprKind::OptionalGet { object, name } => {
                let typed_obj = self.check_expr(object);
                let ty = match &typed_obj.ty {
                    Type::Optional(inner) => {
                        if let Type::Instance(class_name) = &**inner {
                            if let Some(fields) = self.classes.get(class_name) {
                                if let Some(field_ty) = fields.get(name) {
                                    Type::Optional(Box::new(field_ty.clone()))
                                } else {
                                    self.error(expr.span, DiagnosticCode::UnknownIdentifier, &format!("Property '{}' not found on '{}'.", name, class_name));
                                    Type::Error
                                }
                            } else {
                                Type::Error
                            }
                        } else {
                            self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot access property on non-instance optional type '{}'.", inner));
                            Type::Error
                        }
                    }
                    Type::Error | Type::Any => typed_obj.ty.clone(),
                    _ => {
                        self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Optional chaining '?.' requires an optional type, found '{}'.", typed_obj.ty));
                        Type::Error
                    }
                };
                (TypedExprKind::OptionalGet { object: Box::new(typed_obj), name: name.clone() }, ty)
            }
            ExprKind::Array(elements) => {
                if elements.is_empty() {
                    self.error(expr.span, DiagnosticCode::TypeMismatch, "Cannot infer type of empty array literal.");
                    (TypedExprKind::Array(Vec::new()), Type::Error)
                } else {
                    let mut typed_elements = Vec::new();
                    let first_typed = self.check_expr(&elements[0]);
                    let elem_type = first_typed.ty.clone();
                    typed_elements.push(first_typed);
                    
                    for elem in elements.iter().skip(1) {
                        let next_typed = self.check_expr(elem);
                        if next_typed.ty != elem_type && next_typed.ty != Type::Error && elem_type != Type::Error {
                            self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Array elements have inconsistent types: expected '{}', found '{}'.", elem_type, next_typed.ty));
                        }
                        typed_elements.push(next_typed);
                    }
                    (TypedExprKind::Array(typed_elements), Type::Array(Box::new(elem_type)))
                }
            }
            ExprKind::ArrayRepeat { value, count } => {
                let typed_value = self.check_expr(value);
                let typed_count = self.check_expr(count);
                if typed_count.ty != Type::Int && typed_count.ty != Type::Error {
                    self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Array repeat count must be 'Int', found '{}'.", typed_count.ty));
                }
                let ty = Type::Array(Box::new(typed_value.ty.clone()));
                (TypedExprKind::ArrayRepeat { value: Box::new(typed_value), count: Box::new(typed_count) }, ty)
            }
            ExprKind::IndexGet { object, index } => {
                let typed_obj = self.check_expr(object);
                let typed_idx = self.check_expr(index);
                if typed_idx.ty != Type::Int && typed_idx.ty != Type::Error {
                    self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Array index must be 'Int', found '{}'.", typed_idx.ty));
                }
                let ty = match &typed_obj.ty {
                    Type::Array(inner) => (**inner).clone(),
                    Type::Error => Type::Error,
                    _ => {
                        self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot index into non-array type '{}'.", typed_obj.ty));
                        Type::Error
                    }
                };
                (TypedExprKind::IndexGet { object: Box::new(typed_obj), index: Box::new(typed_idx) }, ty)
            }
            ExprKind::IndexSet { object, index, value } => {
                let typed_obj = self.check_expr(object);
                let typed_idx = self.check_expr(index);
                let typed_val = self.check_expr(value);
                
                if typed_idx.ty != Type::Int && typed_idx.ty != Type::Error {
                    self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Array index must be 'Int', found '{}'.", typed_idx.ty));
                }
                
                match &typed_obj.ty {
                    Type::Array(inner) => {
                        if !self.is_assignable(&typed_val.ty, &inner) && typed_val.ty != Type::Error && **inner != Type::Error && **inner != Type::Any {
                            self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot assign type '{}' to array element of type '{}'.", typed_val.ty, inner));
                        }
                    }
                    Type::Error => {}
                    _ => {
                        self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot index into non-array type '{}'.", typed_obj.ty));
                    }
                }
                (TypedExprKind::IndexSet { object: Box::new(typed_obj), index: Box::new(typed_idx), value: Box::new(typed_val.clone()) }, typed_val.ty)
            }
            ExprKind::Get { object, name } => {
                let typed_obj = self.check_expr(object);
                
                let (class_name, instance_args) = match &typed_obj.ty {
                    Type::Instance(n) => (n.clone(), Vec::new()),
                    Type::GenericInstance(n, args) => (n.clone(), args.clone()),
                    Type::Interface(n) => (n.clone(), Vec::new()),
                    Type::Enum(n, _args) => (n.clone(), Vec::new()),
                    _ => {
                        if typed_obj.ty != Type::Error {
                            self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot get property '{}' on non-instance type '{}'.", name, typed_obj.ty))
                        }
                        return TypedExpr::new(TypedExprKind::Get { object: Box::new(typed_obj), name: name.clone() }, Type::Error, expr.span);
                    }
                };
                
                let ty = if let Some(class_props) = self.classes.get(&class_name) {
                    if let Some(prop_ty) = class_props.get(name) {
                        let mut resolved_ty = prop_ty.clone();
                        if let Some(Type::Class(_, params)) = self.env.resolve(&class_name) {
                            let mut inferred_map = std::collections::HashMap::new();
                            for (i, p) in params.iter().enumerate() {
                                if i < instance_args.len() {
                                    inferred_map.insert(p.clone(), instance_args[i].clone());
                                }
                            }
                            resolved_ty = self.substitute_generics(prop_ty, &inferred_map);
                        }
                        resolved_ty
                    } else {
                        self.error(expr.span, DiagnosticCode::UnknownType, &format!("Property '{}' not found on class '{}'.", name, class_name));
                        Type::Error
                    }
                } else if let Some(interface_props) = self.interfaces.get(&class_name) {
                    if let Some(prop_ty) = interface_props.get(name) {
                        prop_ty.clone()
                    } else {
                        self.error(expr.span, DiagnosticCode::UnknownType, &format!("Property '{}' not found on interface '{}'.", name, class_name));
                        Type::Error
                    }
                } else if let Some(enum_variants) = self.enums.get(&class_name) {
                    if let Some(variant_ty) = enum_variants.get(name) {
                        let mut resolved_ty = variant_ty.clone();
                        // Instantiate generic arguments if present
                        if let Type::Enum(_, params) = &typed_obj.ty {
                            let mut inferred_map = std::collections::HashMap::new();
                            for (i, p) in params.iter().enumerate() {
                                if i < instance_args.len() {
                                    inferred_map.insert(p.clone(), instance_args[i].clone());
                                }
                            }
                            resolved_ty = self.substitute_generics(variant_ty, &inferred_map);
                        }
                        return TypedExpr::new(TypedExprKind::EnumVariant {
                            enum_name: class_name.clone(),
                            variant_name: name.clone(),
                        }, resolved_ty, expr.span);
                    } else {
                        self.error(expr.span, DiagnosticCode::UnknownType, &format!("Variant '{}' not found in enum '{}'.", name, class_name));
                        Type::Error
                    }
                } else {
                    self.error(expr.span, DiagnosticCode::UnknownType, &format!("Type '{}' not found.", class_name));
                    Type::Error
                };
                (TypedExprKind::Get { object: Box::new(typed_obj), name: name.clone() }, ty)
            }
            ExprKind::Set { object, name, value } => {
                let typed_obj = self.check_expr(object);
                let typed_val = self.check_expr(value);
                
                let (class_name, instance_args) = match &typed_obj.ty {
                    Type::Instance(n) => (n.clone(), Vec::new()),
                    Type::GenericInstance(n, args) => (n.clone(), args.clone()),
                    Type::Interface(n) => (n.clone(), Vec::new()),
                    _ => {
                        if typed_obj.ty != Type::Error {
                            self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot set property '{}' on non-instance type '{}'.", name, typed_obj.ty))
                        }
                        return TypedExpr::new(TypedExprKind::Set { object: Box::new(typed_obj), name: name.clone(), value: Box::new(typed_val.clone()) }, typed_val.ty, expr.span);
                    }
                };

                if let Some(class_props) = self.classes.get(&class_name) {
                    if let Some(prop_ty) = class_props.get(name) {
                        let mut resolved_ty = prop_ty.clone();
                        if let Some(Type::Class(_, params)) = self.env.resolve(&class_name) {
                            let mut inferred_map = std::collections::HashMap::new();
                            for (i, p) in params.iter().enumerate() {
                                if i < instance_args.len() {
                                    inferred_map.insert(p.clone(), instance_args[i].clone());
                                }
                            }
                            resolved_ty = self.substitute_generics(prop_ty, &inferred_map);
                        }
                        
                        if !self.is_assignable(&typed_val.ty, &resolved_ty) && typed_val.ty != Type::Error && resolved_ty != Type::Error && resolved_ty != Type::Any {
                            self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot assign type '{}' to property of type '{}'.", typed_val.ty, resolved_ty))
                        }
                    } else {
                        self.error(expr.span, DiagnosticCode::UnknownType, &format!("Property '{}' not found on class '{}'.", name, class_name))
                    }
                }
                (TypedExprKind::Set { object: Box::new(typed_obj), name: name.clone(), value: Box::new(typed_val.clone()) }, typed_val.ty)
            }
            ExprKind::Grouping(inner) => {
                let typed_inner = self.check_expr(inner);
                let ty = typed_inner.ty.clone();
                (TypedExprKind::Grouping(Box::new(typed_inner)), ty)
            }
            ExprKind::Match { value, arms } => {
                let typed_value = self.check_expr(value);
                let mut typed_arms = Vec::new();
                let mut common_return_type = None;

                for arm in arms {
                    self.env.push_scope();

                    // Declare bindings in scope
                    match &arm.pattern {
                        ast::Pattern::Wildcard => {}
                        ast::Pattern::Variant { path, bindings } => {
                            if let Some(binds) = bindings {
                                // Extract actual types from the variant
                                let mut extracted_types = Vec::new();
                                
                                let mut enum_name_opt = None;
                                let mut type_args = Vec::new();
                                match &typed_value.ty {
                                    Type::GenericInstance(name, args) => {
                                        enum_name_opt = Some(name.clone());
                                        type_args = args.clone();
                                    }
                                    Type::Instance(name) => {
                                        enum_name_opt = Some(name.clone());
                                    }
                                    _ => {}
                                }
                                
                                if let Some(enum_name) = enum_name_opt {
                                    if let Some(variants) = self.enums.get(&enum_name) {
                                        let variant_name = path.last().unwrap_or(&"".to_string()).clone();
                                        if let Some(Type::EnumVariantConstructor(_, _, func_type_params, param_types, _)) = variants.get(&variant_name) {
                                            // Substitute generics
                                            let mut replacements = std::collections::HashMap::new();
                                            for (tp, actual) in func_type_params.iter().zip(type_args.iter()) {
                                                replacements.insert(tp.clone(), actual.clone());
                                            }
                                            for pt in param_types {
                                                extracted_types.push(self.substitute_generics(pt, &replacements));
                                            }
                                        }
                                    }
                                }
                                
                                for (i, bind) in binds.iter().enumerate() {
                                    if bind != "_" {
                                        let bind_ty = extracted_types.get(i).cloned().unwrap_or(Type::Any);
                                        self.env.declare(bind.clone(), bind_ty);
                                    }
                                }
                            }
                        }
                    }

                    let typed_body = self.check_expr(&arm.body);
                    self.env.pop_scope();

                    if let Some(ref crt) = common_return_type {
                        if !self.is_assignable(&typed_body.ty, crt) && typed_body.ty != Type::Error && *crt != Type::Error {
                            if self.is_assignable(crt, &typed_body.ty) {
                                // Promote crt
                                common_return_type = Some(typed_body.ty.clone());
                            } else {
                                self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Match arms have incompatible return types. Expected '{}', found '{}'.", crt, typed_body.ty));
                            }
                        }
                    } else {
                        common_return_type = Some(typed_body.ty.clone());
                    }

                    typed_arms.push(ast::TypedMatchArm { pattern: arm.pattern.clone(), body: Box::new(typed_body) });
                }

                // Exhaustiveness checking should happen here.

                let ty = common_return_type.unwrap_or(Type::Void);
                (TypedExprKind::Match { value: Box::new(typed_value), arms: typed_arms }, ty)
            }
            ExprKind::Call { callee, type_args, arguments } => {
                let typed_callee = self.check_expr(callee);

                let mut expected_param_types = None;
                match &typed_callee.ty {
                    Type::Function(_, param_types, _) => {
                        expected_param_types = Some(param_types.clone());
                    }
                    Type::EnumVariantConstructor(_, _, _, param_types, _) => {
                        expected_param_types = Some(param_types.clone());
                    }
                    Type::Class(class_name, _) => {
                        if let Some(props) = self.classes.get(class_name) {
                            if let Some(Type::Function(_, param_types, _)) = props.get("init") {
                                expected_param_types = Some(param_types.clone());
                            }
                        }
                        if expected_param_types.is_none() {
                            if let Some(generic_stmt) = self.generic_registry.get_class(class_name).cloned() {
                                if let ast::StmtKind::Class { methods, .. } = &generic_stmt.kind {
                                    for method in methods {
                                        if let ast::StmtKind::Func { name, params, .. } = &method.kind {
                                            if name == "init" {
                                                
                                                for _ in params {
                                                     // Dummy for now, we'd need parse_type which needs mut self
                                                    // Let's just not do Class expected types here for now, 
                                                    // it's not strictly necessary for Enum fixes.
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }

                let mut typed_args = Vec::new();
                let mut arg_types = Vec::new();
                for (i, arg) in arguments.iter().enumerate() {
                    let expected_arg_ty = expected_param_types.as_ref().and_then(|pt| pt.get(i));
                    let typed_arg = self.check_expr_with_expected(arg, expected_arg_ty);
                    arg_types.push(typed_arg.ty.clone());
                    typed_args.push(typed_arg);
                }
                
                let ty = match &typed_callee.ty {
                    Type::BuiltinFunc => Type::Void,
                    Type::Class(class_name, class_type_params) => {
                        let mut constructor_ty = self.classes.get(class_name)
                            .and_then(|props| props.get("init").cloned());
                            
                        if constructor_ty.is_none() {
                            if let Some(generic_stmt) = self.generic_registry.get_class(class_name).cloned() {
                                if let ast::StmtKind::Class { type_params, methods, .. } = &generic_stmt.kind {
                                    self.env.push_scope();
                                    for tp in type_params {
                                        self.env.declare(tp.clone(), Type::Generic(tp.clone()));
                                    }
                                    for method in methods {
                                        if let ast::StmtKind::Func { name, params, .. } = &method.kind {
                                            if name == "init" {
                                                let mut param_types = Vec::new();
                                                for (_, ty) in params {
                                                    param_types.push(self.parse_type(ty, expr.span));
                                                }

                                                constructor_ty = Some(Type::Function(Vec::new(), param_types, Box::new(Type::Void)));
                                            }
                                        }
                                    }
                                    self.env.pop_scope();
                                }
                            }
                        }
                            
                        let mut resolved_type_args = Vec::new();

                        if let Some(Type::Function(_, param_types, _)) = constructor_ty {
                            if param_types.len() != arg_types.len() {
                                self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Constructor expected {} arguments, found {}.", param_types.len(), arg_types.len()))
                            } else {
                                // Basic Local Inference & Checking
                                if !class_type_params.is_empty() {
                                    if type_args.is_empty() {
                                        // Infer from arguments
                                        let mut inferred_map = std::collections::HashMap::new();
                                        for (expected, actual) in param_types.iter().zip(arg_types.iter()) {
                                            self.infer_generics(expected, actual, &mut inferred_map);
                                        }
                                        
                                        for tp in class_type_params {
                                            if let Some(ty) = inferred_map.get(tp) {
                                                resolved_type_args.push(ty.clone());
                                            } else {
                                                self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot infer generic type '{}'. Please provide explicit type arguments.", tp));
                                                resolved_type_args.push(Type::Error);
                                            }
                                        }
                                    } else {
                                        // Explicit arguments provided
                                        if type_args.len() != class_type_params.len() {
                                            self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Expected {} generic arguments, found {}.", class_type_params.len(), type_args.len()))
                                        }
                                        for arg_expr in type_args {
                                            resolved_type_args.push(self.parse_type(arg_expr, expr.span));
                                        }
                                    }
                                }
                                
                                // Instantiate the generic class!
                                if !class_type_params.is_empty() {
                                    let mangled_name = self.instantiate_generic_class(class_name, class_type_params, &resolved_type_args);
                                    
                                    // Rewrite the callee to point to the mangled name!
                                    let new_callee = TypedExpr {
                                        kind: TypedExprKind::Variable(mangled_name.clone()),
                                        ty: Type::Class(mangled_name.clone(), Vec::new()),
                                        span: callee.span,
                                    };
                                    
                                    // We must update ty to be Instance(mangled_name) instead of GenericInstance.
                                    let new_ty = Type::Instance(mangled_name.clone());
                                    
                                    // But wait, we still need to check argument types correctly!
                                    // We can just proceed, because substitute generic parameters handles the check.
                                    
                                    let mut replacements = std::collections::HashMap::new();
                                    for (tp, resolved) in class_type_params.iter().zip(resolved_type_args.iter()) {
                                        replacements.insert(tp.clone(), resolved.clone());
                                    }

                                    for (i, (expected, actual)) in param_types.iter().zip(arg_types.iter()).enumerate() {
                                        let expected_sub = self.substitute_generics(expected, &replacements);
                                        if !self.is_assignable(actual, &expected_sub) {
                                            self.error(arguments[i].span, DiagnosticCode::TypeMismatch, &format!("Expected type '{}' for argument, found '{}'.", expected_sub, actual));
                                        }
                                    }
                                    
                                    return TypedExpr {
                                        kind: TypedExprKind::Call {
                                            callee: Box::new(new_callee),
                                            type_args: Vec::new(),
                                            arguments: typed_args,
                                        },
                                        ty: new_ty,
                                        span: expr.span,
                                    };
                                }
                                
                                // Substitute generic parameters when checking constructor argument types
                                for (i, (expected, actual)) in param_types.iter().zip(arg_types.iter()).enumerate() {
                                    let expected_sub = if class_type_params.is_empty() {
                                        expected.clone()
                                    } else {
                                        let mut type_map = std::collections::HashMap::new();
                                        for (i, p) in class_type_params.iter().enumerate() {
                                            if i < resolved_type_args.len() {
                                                type_map.insert(p.clone(), resolved_type_args[i].clone());
                                            }
                                        }
                                        self.substitute_generics(expected, &type_map)
                                    };
                                    
                                    if !self.is_assignable(actual, &expected_sub) && expected_sub != Type::Any && *actual != Type::Error {
                                        self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Argument {} to constructor expects '{}', found '{}'.", i + 1, expected_sub, actual))
                                    }
                                }
                            }
                        } else if !arg_types.is_empty() {
                            self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Class '{}' has no 'init' method but arguments were provided.", class_name))
                        }
                        
                        if !class_type_params.is_empty() {
                            let mangled_name = self.instantiate_generic_class(class_name, class_type_params, &resolved_type_args);
                            
                            let new_callee = TypedExpr {
                                kind: TypedExprKind::Variable(mangled_name.clone()),
                                ty: Type::BuiltinFunc, // Or Class
                                span: callee.span,
                            };
                            
                            let new_ty = Type::Instance(mangled_name.clone());
                            
                            return TypedExpr {
                                kind: TypedExprKind::Call {
                                    callee: Box::new(new_callee),
                                    type_args: Vec::new(),
                                    arguments: typed_args,
                                },
                                ty: new_ty,
                                span: expr.span,
                            };
                        }
                        
                        Type::Instance(class_name.clone())
                    }
                    Type::Function(func_type_params, param_types, ret_ty) => {
                        let mut inferred_map = std::collections::HashMap::new();

                        if !func_type_params.is_empty() {
                            if type_args.is_empty() {
                                // Infer from arguments
                                for (expected, actual) in param_types.iter().zip(arg_types.iter()) {
                                    self.infer_generics(expected, actual, &mut inferred_map);
                                }
                                
                                // Infer from expected return type (contextual bidirectional inference)
                                if let Some(expected_result) = expected_ty {
                                    self.infer_generics(ret_ty, expected_result, &mut inferred_map);
                                }
                                
                                for tp in func_type_params {
                                    if !inferred_map.contains_key(tp) {
                                        self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot infer generic type '{}'. Please provide explicit type arguments.", tp));
                                        inferred_map.insert(tp.clone(), Type::Error);
                                    }
                                }
                            } else {
                                if type_args.len() != func_type_params.len() {
                                    self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Expected {} generic arguments, found {}.", func_type_params.len(), type_args.len()))
                                }
                                for (i, arg_expr) in type_args.iter().enumerate() {
                                    let ty = self.parse_type(arg_expr, expr.span);
                                    if i < func_type_params.len() {
                                        inferred_map.insert(func_type_params[i].clone(), ty);
                                    }
                                }
                            }
                        }

                        if param_types.len() != arg_types.len() {
                            self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Expected {} arguments, found {}.", param_types.len(), arg_types.len()))
                        } else {
                            for (i, (expected, actual)) in param_types.iter().zip(arg_types.iter()).enumerate() {
                                let expected_sub = if func_type_params.is_empty() {
                                    expected.clone()
                                } else {
                                    self.substitute_generics(expected, &inferred_map)
                                };
                                
                                if !self.is_assignable(actual, &expected_sub) && expected_sub != Type::Any && *actual != Type::Error {
                                    self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Argument {} expected type '{}', found '{}'.", i + 1, expected_sub, actual))
                                }
                            }
                        }
                        
                        if func_type_params.is_empty() {
                            (**ret_ty).clone()
                        } else {
                            let mut resolved_type_args = Vec::new();
                            for tp in func_type_params {
                                resolved_type_args.push(inferred_map.get(tp).unwrap_or(&Type::Error).clone());
                            }
                            
                            if let TypedExprKind::Variable(func_name) = &typed_callee.kind {
                                let mangled_name = self.instantiate_generic_function(func_name, func_type_params, &resolved_type_args);
                                
                                let new_callee = TypedExpr {
                                    kind: TypedExprKind::Variable(mangled_name.clone()),
                                    ty: Type::BuiltinFunc, // Can be treated as builtin or regular function
                                    span: callee.span,
                                };
                                
                                let new_ret_ty = self.substitute_generics(ret_ty, &inferred_map);
                                
                                return TypedExpr {
                                    kind: TypedExprKind::Call {
                                        callee: Box::new(new_callee),
                                        type_args: Vec::new(),
                                        arguments: typed_args,
                                    },
                                    ty: new_ret_ty,
                                    span: expr.span,
                                };
                            }
                            
                            self.substitute_generics(ret_ty, &inferred_map)
                        }
                    }
                    Type::EnumVariantConstructor(enum_name, variant_name, func_type_params, param_types, ret_ty) => {
                        let mut inferred_map = std::collections::HashMap::new();

                        if !func_type_params.is_empty() {
                            if type_args.is_empty() {
                                // Infer from arguments
                                for (expected, actual) in param_types.iter().zip(arg_types.iter()) {
                                    self.infer_generics(expected, actual, &mut inferred_map);
                                }
                                
                                // Infer from expected return type (contextual bidirectional inference)
                                if let Some(expected_result) = expected_ty {
                                    self.infer_generics(ret_ty, expected_result, &mut inferred_map);
                                }
                                
                                for tp in func_type_params {
                                    if !inferred_map.contains_key(tp) {
                                        self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot infer generic type '{}'. Please provide explicit type arguments.", tp));
                                        inferred_map.insert(tp.clone(), Type::Error);
                                    }
                                }
                            } else {
                                if type_args.len() != func_type_params.len() {
                                    self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Expected {} generic arguments, found {}.", func_type_params.len(), type_args.len()))
                                }
                                for (i, arg_expr) in type_args.iter().enumerate() {
                                    let ty = self.parse_type(arg_expr, expr.span);
                                    if i < func_type_params.len() {
                                        inferred_map.insert(func_type_params[i].clone(), ty);
                                    }
                                }
                            }
                        }

                        if param_types.len() != arg_types.len() {
                            self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Expected {} arguments, found {}.", param_types.len(), arg_types.len()))
                        } else {
                            for (i, (expected, actual)) in param_types.iter().zip(arg_types.iter()).enumerate() {
                                let expected_sub = if func_type_params.is_empty() {
                                    expected.clone()
                                } else {
                                    self.substitute_generics(expected, &inferred_map)
                                };
                                
                                if !self.is_assignable(actual, &expected_sub) && expected_sub != Type::Any && *actual != Type::Error {
                                    self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Argument {} expected type '{}', found '{}'.", i + 1, expected_sub, actual))
                                }
                            }
                        }
                        
                        let new_ret_ty = if func_type_params.is_empty() {
                            (**ret_ty).clone()
                        } else {
                            self.substitute_generics(ret_ty, &inferred_map)
                        };

                        return TypedExpr {
                            kind: TypedExprKind::Call {
                                callee: Box::new(TypedExpr {
                                    kind: TypedExprKind::EnumVariant {
                                        enum_name: enum_name.clone(),
                                        variant_name: variant_name.clone(),
                                    },
                                    ty: Type::BuiltinFunc,
                                    span: callee.span,
                                }),
                                type_args: Vec::new(),
                                arguments: typed_args,
                            },
                            ty: new_ret_ty.clone(),
                            span: expr.span,
                        };
                    }
                    Type::Error => Type::Error,
                    _ => {
                        self.error(expr.span, DiagnosticCode::TypeMismatch, "Cannot call non-function type.");
                        Type::Error
                    }
                };
                (TypedExprKind::Call { callee: Box::new(typed_callee), type_args: type_args.clone(), arguments: typed_args }, ty)
            }
            ExprKind::Unary(op, right) => {
                let typed_right = self.check_expr(right);
                if typed_right.ty == Type::Error {
                    return TypedExpr::new(TypedExprKind::Unary(op.clone(), Box::new(typed_right)), Type::Error, expr.span);
                }

                let ty = match op {
                    UnaryOp::Negate => {
                        if typed_right.ty == Type::Int || typed_right.ty == Type::Float {
                            typed_right.ty.clone()
                        } else {
                            self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot negate type '{}'.", typed_right.ty));
                            Type::Error
                        }
                    }
                };
                (TypedExprKind::Unary(op.clone(), Box::new(typed_right)), ty)
            }
            ExprKind::Binary(left, op, right) => {
                let typed_left = self.check_expr(left);
                let typed_right = self.check_expr(right);

                if typed_left.ty == Type::Error || typed_right.ty == Type::Error {
                    return TypedExpr::new(TypedExprKind::Binary(Box::new(typed_left), op.clone(), Box::new(typed_right)), Type::Error, expr.span);
                }

                let ty = match op {
                    BinaryOp::Add | BinaryOp::Subtract | BinaryOp::Multiply | BinaryOp::Divide => {
                        if (self.is_assignable(&typed_left.ty, &Type::Int) || typed_left.ty == Type::Float) && self.is_assignable(&typed_left.ty, &typed_right.ty) {
                            typed_left.ty.clone()
                        } else if *op == BinaryOp::Add && typed_left.ty == Type::String && typed_right.ty == Type::String {
                            Type::String
                        } else {
                            self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot apply operator to types '{}' and '{}'.", typed_left.ty, typed_right.ty));
                            Type::Error
                        }
                    }
                    BinaryOp::Equal | BinaryOp::NotEqual => {
                        if !self.is_assignable(&typed_left.ty, &typed_right.ty) && !self.is_assignable(&typed_right.ty, &typed_left.ty) {
                            self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot compare types '{}' and '{}' for equality.", typed_left.ty, typed_right.ty));
                            Type::Error
                        } else {
                            Type::Boolean
                        }
                    }
                    BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual => {
                        if !self.is_assignable(&typed_left.ty, &typed_right.ty) && !self.is_assignable(&typed_right.ty, &typed_left.ty) {
                            self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot apply comparison to types '{}' and '{}'.", typed_left.ty, typed_right.ty));
                        }
                        Type::Boolean
                    }
                };
                (TypedExprKind::Binary(Box::new(typed_left), op.clone(), Box::new(typed_right)), ty)
            }
        };
        TypedExpr::new(kind, ty, expr.span)
    }

    fn parse_type(&mut self, type_expr: &TypeExpr, span: Span) -> Type {
        match type_expr {
            TypeExpr::Named(name) => match name.as_str() {
                "Int" => Type::Int,
                "Float" => Type::Float,
                "String" => Type::String,
                "Boolean" => Type::Boolean,
                "Void" => Type::Void,
                "CInt" => Type::CInt,
                "CUInt" => Type::CUInt,
                "CChar" => Type::CChar,
                "CSize" => Type::CSize,
                _ => {
                    if let Some(Type::Generic(g)) = self.env.resolve(name) {
                        return Type::Generic(g.clone());
                    }
                    if self.classes.contains_key(name) || self.enums.contains_key(name) {
                        Type::Instance(name.to_string())
                    } else if self.interfaces.contains_key(name) {
                        Type::Interface(name.to_string())
                    } else {
                        self.error(span, DiagnosticCode::UnknownType, &format!("Unknown type '{}'.", name));

                        Type::Error
                    }
                }
            },
            TypeExpr::GenericInstance(name, args) => {
                let parsed_args = args.iter().map(|a| self.parse_type(a, span)).collect::<Vec<_>>();
                if name == "Pointer" && parsed_args.len() == 1 {
                    Type::Pointer(Box::new(parsed_args[0].clone()))
                } else if self.classes.contains_key(name) || self.enums.contains_key(name) {
                    Type::GenericInstance(name.clone(), parsed_args)
                } else {
                    self.error(span, DiagnosticCode::UnknownType, &format!("Unknown generic class '{}'.", name));

                    Type::Error
                }
            }
            TypeExpr::Optional(inner) => {
                Type::Optional(Box::new(self.parse_type(inner, span)))
            }
            TypeExpr::Array(inner) => {
                Type::Array(Box::new(self.parse_type(inner, span)))
            }
        }
    }

    fn is_assignable(&self, source: &Type, target: &Type) -> bool {
        if source == target {
            return true;
        }
        
        let is_source_int = matches!(source, Type::Int | Type::CInt | Type::CUInt | Type::CChar | Type::CSize);
        let is_target_int = matches!(target, Type::Int | Type::CInt | Type::CUInt | Type::CChar | Type::CSize);
        if is_source_int && is_target_int {
            return true;
        }
        if *source == Type::Null {
            if matches!(target, Type::Optional(_)) {
                return true;
            }
        }
        if let Type::Optional(inner) = target {
            if self.is_assignable(source, inner) {
                return true;
            }
        }
        if *target == Type::Any {
            return true;
        }
        if let (Type::Instance(class_name), Type::Interface(interface_name)) = (source, target) {
            if let Some(implements) = self.class_implements.get(class_name) {
                if implements.contains(interface_name) {
                    return true;
                }
            }
        }
        false
    }

    fn substitute_generics(&self, ty: &Type, replacements: &std::collections::HashMap<String, Type>) -> Type {
        match ty {
            Type::Generic(g) => {
                if let Some(replacement) = replacements.get(g) {
                    replacement.clone()
                } else {
                    ty.clone()
                }
            }
            Type::Optional(inner) => Type::Optional(Box::new(self.substitute_generics(inner, replacements))),
            Type::Array(inner) => Type::Array(Box::new(self.substitute_generics(inner, replacements))),
            Type::Function(type_params, params, ret) => {
                let sub_params = params.iter().map(|p| self.substitute_generics(p, replacements)).collect();
                let sub_ret = Box::new(self.substitute_generics(ret, replacements));
                Type::Function(type_params.clone(), sub_params, sub_ret)
            }
            Type::GenericInstance(name, args) => {
                let sub_args = args.iter().map(|a| self.substitute_generics(a, replacements)).collect();
                Type::GenericInstance(name.clone(), sub_args)
            }
            _ => ty.clone(),
        }
    }

    fn infer_generics(&self, expected: &Type, actual: &Type, inferred_map: &mut std::collections::HashMap<String, Type>) {
        match (expected, actual) {
            (Type::Generic(g), _) => {
                if let std::collections::hash_map::Entry::Vacant(e) = inferred_map.entry(g.clone()) {
                    e.insert(actual.clone());
                }
            }
            (Type::Optional(e), Type::Optional(a)) => self.infer_generics(e, a, inferred_map),
            (Type::Array(e), Type::Array(a)) => self.infer_generics(e, a, inferred_map),
            (Type::GenericInstance(e_name, e_args), Type::GenericInstance(a_name, a_args)) if e_name == a_name => {
                for (e_arg, a_arg) in e_args.iter().zip(a_args.iter()) {
                    self.infer_generics(e_arg, a_arg, inferred_map);
                }
            }
            (Type::Enum(e_name, e_params), Type::GenericInstance(a_name, a_args)) | (Type::Class(e_name, e_params), Type::GenericInstance(a_name, a_args)) if e_name == a_name => {
                for (e_param, a_arg) in e_params.iter().zip(a_args.iter()) {
                    self.infer_generics(&Type::Generic(e_param.clone()), a_arg, inferred_map);
                }
            }
            (Type::Function(_, e_params, e_ret), Type::Function(_, a_params, a_ret)) => {
                for (e_param, a_param) in e_params.iter().zip(a_params.iter()) {
                    self.infer_generics(e_param, a_param, inferred_map);
                }
                self.infer_generics(e_ret, a_ret, inferred_map);
            }
            _ => {}
        }
    }

    fn error(&mut self, span: Span, code: DiagnosticCode, message: &str) {
        eprintln!("TYPECHECKER ERROR: {} at {:?}", message, span);
        self.errors.push(DiagnosticBuilder::error(code, message, span).build());
    }

    fn type_to_type_expr(ty: &Type) -> ast::TypeExpr {
        match ty {
            Type::Int => ast::TypeExpr::Named("Int".to_string()),
            Type::Float => ast::TypeExpr::Named("Float".to_string()),
            Type::Boolean => ast::TypeExpr::Named("Boolean".to_string()),
            Type::String => ast::TypeExpr::Named("String".to_string()),
            Type::Instance(name) | Type::Interface(name) => ast::TypeExpr::Named(name.clone()),
            Type::GenericInstance(name, args) => {
                ast::TypeExpr::GenericInstance(name.clone(), args.iter().map(Self::type_to_type_expr).collect())
            }
            Type::Optional(inner) => ast::TypeExpr::Optional(Box::new(Self::type_to_type_expr(inner))),
            Type::Array(inner) => ast::TypeExpr::Array(Box::new(Self::type_to_type_expr(inner))),
            _ => ast::TypeExpr::Named("Any".to_string()),
        }
    }

    fn instantiate_generic_class(&mut self, class_name: &str, type_params: &[String], type_args: &[Type]) -> String {
        let type_arg_strings: Vec<String> = type_args.iter().map(|t| format!("{}", t)).collect();
        let key = generics::SpecializationKey::new(class_name.to_string(), type_arg_strings);
        let mangled_name = key.mangled_name();

        if self.spec_registry.get_state(&key) == Some(&generics::SpecializationState::Complete) {
            return mangled_name;
        }
        if self.spec_registry.get_state(&key) == Some(&generics::SpecializationState::Pending) {
            return mangled_name; // Break recursion
        }

        self.spec_registry.mark_pending(key.clone());

        if let Some(generic_stmt) = self.generic_registry.get_class(class_name).cloned() {
            let mut type_arg_exprs = Vec::new();
            for ty in type_args {
                type_arg_exprs.push(Self::type_to_type_expr(ty));
            }

            let substitution = generics::TypeSubstitution::new(type_params, &type_arg_exprs);
            let monomorphizer = generics::Monomorphizer::new(&substitution, mangled_name.clone());
            
            let concrete_stmt = monomorphizer.monomorphize_stmt(&generic_stmt);
            self.spec_registry.mark_complete(key);
            
            // Eagerly typecheck the generated class so it's immediately available to the caller
            eprintln!("Eagerly typechecking {}", mangled_name);
            let typed_stmt = self.check_stmt(&concrete_stmt);
            eprintln!("Finished eagerly typechecking {}", mangled_name);
            self.pending_instantiations.push(typed_stmt);
        } else {
            // Error handling if generic not found? Should never happen because checker knows about it.
        }
        
        mangled_name
    }

    fn instantiate_generic_function(&mut self, func_name: &str, type_params: &[String], type_args: &[Type]) -> String {
        let type_arg_strings: Vec<String> = type_args.iter().map(|t| format!("{}", t)).collect();
        let key = generics::SpecializationKey::new(func_name.to_string(), type_arg_strings);
        let mangled_name = key.mangled_name();

        if self.spec_registry.get_state(&key) == Some(&generics::SpecializationState::Complete) {
            return mangled_name;
        }
        if self.spec_registry.get_state(&key) == Some(&generics::SpecializationState::Pending) {
            return mangled_name; // Break recursion
        }

        self.spec_registry.mark_pending(key.clone());

        if let Some(generic_stmt) = self.generic_registry.get_function(func_name).cloned() {
            let mut type_arg_exprs = Vec::new();
            for ty in type_args {
                type_arg_exprs.push(Self::type_to_type_expr(ty));
            }

            let substitution = generics::TypeSubstitution::new(type_params, &type_arg_exprs);
            let monomorphizer = generics::Monomorphizer::new(&substitution, mangled_name.clone());
            
            let mut concrete_stmt = monomorphizer.monomorphize_stmt(&generic_stmt);
            if let ast::StmtKind::Func { name, .. } = &mut concrete_stmt.kind {
                *name = mangled_name.clone();
            }
            
            self.spec_registry.mark_complete(key);
            
            // Eagerly typecheck the generated function so it's immediately available to the caller
            let typed_stmt = self.check_stmt(&concrete_stmt);
            self.pending_instantiations.push(typed_stmt);
        } else {
            // Error handling if generic not found? Should never happen because checker knows about it.
        }
        
        mangled_name
    }
    fn get_assigned_properties_in_init(stmt: &TypedStmt) -> std::collections::HashSet<String> {
        let mut assigned = std::collections::HashSet::new();
        match &stmt.kind {
            TypedStmtKind::Block(stmts) => {
                for s in stmts {
                    assigned.extend(Self::get_assigned_properties_in_init(s));
                }
            }
            TypedStmtKind::Expression(expr) => {
                if let TypedExprKind::Set { object, name, value: _ } = &expr.kind {
                    if let TypedExprKind::SelfRef = &object.kind {
                        assigned.insert(name.clone());
                    }
                }
            }
            TypedStmtKind::If { then_branch, else_branch, .. } => {
                let then_assigned = Self::get_assigned_properties_in_init(then_branch);
                if let Some(else_b) = else_branch {
                    let else_assigned = Self::get_assigned_properties_in_init(else_b);
                    // Only count if assigned in BOTH branches
                    for prop in then_assigned {
                        if else_assigned.contains(&prop) {
                            assigned.insert(prop);
                        }
                    }
                }
            }
            TypedStmtKind::While { body: _, .. } | TypedStmtKind::For { body: _, .. } => {
                // Loop bodies might not execute, so we don't count assignments inside them as definite!
            }
            _ => {}
        }
        assigned
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ast::Location;

    fn make_span() -> Span {
        Span::new(0, 0, Location::new(1, 1), Location::new(1, 1))
    }

    #[test]
    fn test_valid_math() {
        let mut checker = TypeChecker::new();
        // let x = 10 + 5;
        let stmt = Stmt::new(StmtKind::Let {
            name: "x".into(),
            is_private: false,
            type_annotation: None,
            initializer: Some(Expr::new(ExprKind::Binary(
                Box::new(Expr::new(ExprKind::Integer(10), make_span())),
                BinaryOp::Add,
                Box::new(Expr::new(ExprKind::Integer(5), make_span())),
            ), make_span())),
        }, make_span());

        checker.check(&[stmt]);
        assert!(checker.errors.is_empty());
        assert_eq!(checker.env.resolve("x").unwrap(), Type::Int);
    }

    #[test]
    fn test_type_mismatch() {
        let mut checker = TypeChecker::new();
        // let x = 10 + "hello";
        let stmt = Stmt::new(StmtKind::Let {
            name: "x".into(),
            is_private: false,
            type_annotation: None,
            initializer: Some(Expr::new(ExprKind::Binary(
                Box::new(Expr::new(ExprKind::Integer(10), make_span())),
                BinaryOp::Add,
                Box::new(Expr::new(ExprKind::String("hello".into()), make_span())),
            ), make_span())),
        }, make_span());

        checker.check(&[stmt]);
        assert_eq!(checker.errors.len(), 1);
        assert!(checker.errors[0].message.contains("Cannot apply operator to types 'Int' and 'String'"));
    }

    #[test]
    fn test_if_condition_type() {
        let mut checker = TypeChecker::new();
        // if 10 { }
        let stmt = Stmt::new(StmtKind::If {
            condition: Expr::new(ExprKind::Integer(10), make_span()),
            then_branch: Box::new(Stmt::new(StmtKind::Block(vec![]), make_span())),
            else_branch: None,
        }, make_span());

        checker.check(&[stmt]);
        assert_eq!(checker.errors.len(), 1);
        assert!(checker.errors[0].message.contains("Expected 'Boolean'"));
    }
}
