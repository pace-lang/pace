use module::graph::ModuleGraph;
use ast::{Expr, ExprKind, Stmt, StmtKind, Span, BinaryOp, UnaryOp, TypeExpr, TypedExpr, TypedExprKind, TypedStmt, TypedStmtKind};
use ast::types::Type;
use crate::env::TypeEnvironment;
use std::collections::HashMap;
use diagnostics::{Diagnostic, DiagnosticBuilder, DiagnosticCode};

pub struct TypeChecker<'a> {
    pub session: &'a mut session::CompilerSession,
    env: TypeEnvironment,
    pub errors: Vec<Diagnostic>,
    current_return_type: Option<Type>,
    pub classes: HashMap<String, HashMap<String, Type>>,
    pub class_mutables: HashMap<String, HashMap<String, bool>>,
    pub interfaces: HashMap<String, HashMap<String, Type>>,
    pub enums: HashMap<String, HashMap<String, Type>>,
    pub class_implements: HashMap<String, Vec<String>>,
    current_class: Option<String>,
    pub generic_registry: generics::GenericDefinitionRegistry,
    pub spec_registry: generics::SpecializationRegistry,
    pub pending_instantiations: Vec<TypedStmt>,
    pub uninitialized_class_properties: HashMap<String, Vec<String>>,
    pub is_checking_method: bool,
}

impl<'a> TypeChecker<'a> {
    pub fn new(session: &'a mut session::CompilerSession) -> Self {
        Self {
            session,
            env: TypeEnvironment::new(),
            errors: Vec::new(),
            current_return_type: None,
            classes: HashMap::new(),
            class_mutables: HashMap::new(),
            interfaces: HashMap::new(),
            enums: HashMap::new(),
            class_implements: HashMap::new(),
            current_class: None,
            generic_registry: generics::GenericDefinitionRegistry::new(),
            spec_registry: generics::SpecializationRegistry::new(),
            pending_instantiations: Vec::new(),
            uninitialized_class_properties: HashMap::new(),
            is_checking_method: false,
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
            StmtKind::Let { name, type_annotation, initializer, is_private: _ } => self.check_var_decl(*name, type_annotation, initializer, false, false, stmt.span).kind,
            StmtKind::Var { name, type_annotation, initializer, is_weak, is_private: _ } => self.check_var_decl(*name, type_annotation, initializer, *is_weak, true, stmt.span).kind,
            StmtKind::Class { name, type_params, implements, methods, fields, is_private: _ } => {
                if !type_params.is_empty() {
                    self.generic_registry.register_class(self.session.interner.lookup(*name).to_string(), stmt.clone());
                    return TypedStmt { kind: TypedStmtKind::Block(Vec::new()), span: stmt.span };
                }

                self.env.declare(self.session.interner.lookup(*name).to_string(), Type::Class(self.session.interner.lookup(*name).to_string(), type_params.iter().map(|s| self.session.interner.lookup(*s).to_string()).collect()));
                self.classes.insert(self.session.interner.lookup(*name).to_string(), std::collections::HashMap::new());
                
                self.env.push_scope();
                for tp in type_params {
                    self.env.declare(self.session.interner.lookup(*tp).to_string(), Type::Generic(self.session.interner.lookup(*tp).to_string()));
                }

                let mut class_members = HashMap::new();
                let mut uninit_props = Vec::new();
                let mut class_mutables_map = std::collections::HashMap::new();
                
                for field in fields {
                    let (f_name, type_annotation, initializer, is_weak, is_mutable) = match &field.kind {
                        StmtKind::Var { name, type_annotation, initializer, is_weak, is_private: _ } => (name, type_annotation, initializer, *is_weak, true),
                        StmtKind::Let { name, type_annotation, initializer, is_private: _ } => (name, type_annotation, initializer, false, false),
                        _ => continue,
                    };
                    
                    if initializer.is_none() {
                        uninit_props.push(self.session.interner.lookup(*f_name).to_string());
                    }
                    
                    let ty = if let Some(ann) = type_annotation {
                        let parsed = self.parse_type(ann, field.span);
                        if is_weak
                            && !matches!(parsed, Type::Optional(ref inner) if matches!(**inner, Type::Instance(_) | Type::Interface(_))) {
                                self.error(field.span, DiagnosticCode::TypeMismatch, "Weak properties must be of optional instance type (e.g. 'weak var x: User?').");
                            }
                        parsed
                    } else if let Some(init) = initializer {
                        let parsed = self.check_expr(init);
                        if is_weak
                            && !matches!(parsed.ty, Type::Optional(ref inner) if matches!(**inner, Type::Instance(_) | Type::Interface(_))) {
                                self.error(field.span, DiagnosticCode::TypeMismatch, "Weak properties must be of optional instance type (e.g. 'weak var x: User?').");
                            }
                        parsed.ty.clone()
                    } else {
                        if is_weak {
                            self.error(field.span, DiagnosticCode::TypeMismatch, "Weak properties must be of optional instance type (e.g. 'weak var x: User?').");
                        }
                        Type::Any
                    };
                    class_members.insert(self.session.interner.lookup(*f_name).to_string(), ty);
                    class_mutables_map.insert(self.session.interner.lookup(*f_name).to_string(), is_mutable);
                }

                self.uninitialized_class_properties.insert(self.session.interner.lookup(*name).to_string(), uninit_props);

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
                        class_members.insert(self.session.interner.lookup(*m_name).to_string(), Type::Function(Vec::new(), param_types, Box::new(ret_ty)));
                    }
                }
                
                self.classes.insert(self.session.interner.lookup(*name).to_string(), class_members.clone());
                self.class_mutables.insert(self.session.interner.lookup(*name).to_string(), class_mutables_map);
                self.class_implements.insert(self.session.interner.lookup(*name).to_string(), implements.iter().map(|s| self.session.interner.lookup(*s).to_string()).collect());
                
                // Validate implements
                for interface_name in implements {
                    if let Some(interface_members) = self.interfaces.get(&self.session.interner.lookup(*interface_name).to_string()).cloned() {
                        for (i_method_name, i_method_ty) in interface_members {
                            if let Some(c_method_ty) = class_members.get(&i_method_name) {
                                if *c_method_ty != i_method_ty {
                                    self.error(stmt.span, DiagnosticCode::TypeMismatch, &format!("Class \'{}\' incorrectly implements method '{}' of interface '{}'. Expected '{}', found '{}'.", self.session.interner.lookup(*name), i_method_name, self.session.interner.lookup(*interface_name), i_method_ty, c_method_ty))
                                }
                            } else {
                                self.error(stmt.span, DiagnosticCode::TypeMismatch, &format!("Class \'{}\' does not implement required method '{}' of interface '{}'.", self.session.interner.lookup(*name), i_method_name, self.session.interner.lookup(*interface_name)))
                            }
                        }
                    } else {
                        self.error(stmt.span, DiagnosticCode::UnknownType, &format!("Interface \'{}\' not found.", self.session.interner.lookup(*interface_name)))
                    }
                }
                
                let prev_class = self.current_class.clone();
                self.current_class = Some(self.session.interner.lookup(*name).to_string());
                
                let mut typed_methods = Vec::new();
                for method in methods {
                    let prev = self.is_checking_method;
                    self.is_checking_method = true;
                    typed_methods.push(self.check_stmt(method));
                    self.is_checking_method = prev;
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
                        interface_methods.insert(self.session.interner.lookup(*m_name).to_string(), Type::Function(Vec::new(), param_types, Box::new(ret_ty)));
                    }
                }
                
                self.interfaces.insert(self.session.interner.lookup(*name).to_string(), interface_methods);
                self.env.declare(self.session.interner.lookup(*name).to_string(), Type::Interface(self.session.interner.lookup(*name).to_string()));
                
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

                self.env.declare(self.session.interner.lookup(*name).to_string(), Type::Enum(self.session.interner.lookup(*name).to_string(), type_params.iter().map(|s| self.session.interner.lookup(*s).to_string()).collect()));
                
                self.env.push_scope();
                for tp in type_params {
                    self.env.declare(self.session.interner.lookup(*tp).to_string(), Type::Generic(self.session.interner.lookup(*tp).to_string()));
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
                        Type::Instance(self.session.interner.lookup(*name).to_string())
                    } else {
                        let mut ret_args = Vec::new();
                        for tp in type_params {
                            ret_args.push(Type::Generic(self.session.interner.lookup(*tp).to_string()));
                        }
                        Type::GenericInstance(self.session.interner.lookup(*name).to_string(), ret_args)
                    };
                    
                    let variant_ty = Type::EnumVariantConstructor(self.session.interner.lookup(*name).to_string(), self.session.interner.lookup(variant.name).to_string(), type_params.iter().map(|s| self.session.interner.lookup(*s).to_string()).collect(), param_types, Box::new(ret_ty));
                    
                    enum_variants.insert(self.session.interner.lookup(variant.name).to_string(), variant_ty);
                }
                
                self.env.pop_scope();
                
                // Declare variants in the enclosing scope
                for (variant_name, variant_ty) in &enum_variants {
                    self.env.declare(variant_name.clone(), variant_ty.clone());
                }
                
                self.enums.insert(self.session.interner.lookup(*name).to_string(), enum_variants);

                TypedStmtKind::Enum {
                    name: *name,
                    type_params: type_params.clone(),
                    variants: variants.clone(),
                }
            }
            StmtKind::ForeignFunc { name, type_params, params, return_type, is_private: _ } => {
                self.env.push_scope();
                for tp in type_params {
                    self.env.declare(self.session.interner.lookup(*tp).to_string(), Type::Generic(self.session.interner.lookup(*tp).to_string()));
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
                
                self.env.declare(self.session.interner.lookup(*name).to_string(), Type::Function(type_params.iter().map(|s| self.session.interner.lookup(*s).to_string()).collect(), param_types.clone(), Box::new(ret_ty.clone())));
                TypedStmtKind::ForeignFunc {
                    name: name.clone(),
                    params: params.clone(),
                    return_type: return_type.clone(),
                }
            }
            StmtKind::Func { name, type_params, params, return_type, body, is_private: _ } => {
                if !type_params.is_empty() {
                    self.generic_registry.register_function(self.session.interner.lookup(*name).to_string(), stmt.clone());
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

                let is_method = self.is_checking_method;
                self.is_checking_method = false; // Reset so nested functions are declared as normal variables

                let mut resolved_name = name.clone();
                if !is_method {
                    let func_ty = Type::Function(type_params.iter().map(|s| self.session.interner.lookup(*s).to_string()).collect(), param_types.clone(), Box::new(ret_ty.clone()));
                    if let Some(existing) = self.env.resolve(self.session.interner.lookup(*name)) {
                        if matches!(existing, Type::Function(..) | Type::OverloadedFunction(..)) {
                            let mut funcs = match existing {
                                Type::OverloadedFunction(fs) => fs,
                                Type::Function(..) => vec![(self.session.interner.lookup(*name).to_string(), existing)],
                                _ => unreachable!(),
                            };

                            let mut mangled = format!("_PO_{}", self.session.interner.lookup(*name));
                            for ty in &param_types {
                                mangled.push_str(&format!("_{}", ty).replace("<", "_").replace(">", "").replace(" ", "").replace("?", "Opt").replace("[]", "Arr"));
                            }

                            funcs.push((mangled.clone(), func_ty.clone()));
                            self.env.declare(self.session.interner.lookup(*name).to_string(), Type::OverloadedFunction(funcs));
                            self.env.declare(mangled.clone(), func_ty);
                            resolved_name = self.session.interner.intern(&mangled);
                        } else {
                            self.env.declare(self.session.interner.lookup(*name).to_string(), func_ty);
                        }
                    } else {
                        self.env.declare(self.session.interner.lookup(*name).to_string(), func_ty);
                    }
                }

                self.env.push_scope();
                for tp in type_params {
                    self.env.declare(self.session.interner.lookup(*tp).to_string(), Type::Generic(self.session.interner.lookup(*tp).to_string()));
                }

                
                if let Some(ref class_name) = self.current_class {
                    self.env.declare_var("self".to_string(), Type::Instance(class_name.clone()), false);
                }

                for ((param_name, _), param_ty) in params.iter().zip(param_types) {
                    self.env.declare_var(self.session.interner.lookup(*param_name).to_string(), param_ty, false);
                }

                let previous_return = self.current_return_type.take();
                self.current_return_type = Some(ret_ty.clone());

                let typed_body = self.check_stmt(body);

                if self.session.interner.lookup(*name) == "init"
                    && let Some(ref class_name) = self.current_class
                        && let Some(uninit_props_ref) = self.uninitialized_class_properties.get(class_name) {
                            let uninit_props = uninit_props_ref.clone();
                            let assigned_props = Self::get_assigned_properties_in_init(&typed_body);
                            for prop in uninit_props {
                                if !assigned_props.contains(&self.session.interner.intern(&prop)) {
                                    self.error(stmt.span, DiagnosticCode::UninitializedVariable, &format!("Property '{}' is not initialized by the constructor.", prop));
                                }
                            }
                        }

                self.current_return_type = previous_return;
                self.env.pop_scope();
                TypedStmtKind::Func {
                    name: resolved_name,
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
                let typed_else = else_branch.as_ref().map(|e_branch| Box::new(self.check_stmt(e_branch)));
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
                
                let item_type = match &typed_iterator.ty {
                    Type::Range => Type::Int,
                    Type::Array(inner) => *inner.clone(),
                    Type::Error => Type::Error,
                    _ => {
                        self.error(stmt.span, DiagnosticCode::TypeMismatch, &format!("Cannot iterate over non-iterable type '{}'.", typed_iterator.ty));
                        Type::Error
                    }
                };

                self.env.push_scope();
                self.env.declare_var(self.session.interner.lookup(*item_name).to_string(), item_type, false);
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

    fn check_var_decl(&mut self, name: session::Symbol, type_annotation: &Option<TypeExpr>, initializer: &Option<Expr>, is_weak: bool, is_mutable: bool, span: Span) -> TypedStmt {
        let expected_ty = type_annotation.as_ref().map(|ann| self.parse_type(ann, span));
        
        let typed_init = initializer.as_ref().map(|init| self.check_expr_with_expected(init, expected_ty.as_ref()));
        
        let init_type = typed_init.as_ref().map(|t| &t.ty).unwrap_or(&Type::Any).clone();
        
        let decl_type = if let Some(ann_type) = expected_ty {
            if init_type != Type::Any && !self.is_assignable(&init_type, &ann_type) && init_type != Type::Error {
                self.error(span, DiagnosticCode::TypeMismatch, &format!("Cannot assign type '{}' to variable of type '{}'.", init_type, ann_type));
            }
            
            if is_weak
                && !matches!(ann_type, Type::Optional(ref inner) if matches!(**inner, Type::Instance(_) | Type::Interface(_))) {
                    self.error(span, DiagnosticCode::TypeMismatch, "Weak variables must be of optional instance type (e.g. 'weak var x: User?').");
                }
            ann_type
        } else {
            if is_weak
                 && !matches!(init_type, Type::Optional(ref inner) if matches!(**inner, Type::Instance(_) | Type::Interface(_))) {
                     self.error(span, DiagnosticCode::TypeMismatch, "Weak variables must be of optional instance type (e.g. 'weak var x: User?').");
                 }
            init_type
        };
        
        self.env.declare_var(self.session.interner.lookup(name).to_string(), decl_type, is_mutable);
        
        let kind = if is_weak || (initializer.is_none() && type_annotation.is_some()) {
            TypedStmtKind::Var {
                name,
                type_annotation: type_annotation.clone(),
                initializer: typed_init,
                is_weak,
            }
        } else {
            TypedStmtKind::Let {
                name,
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
            ExprKind::InterpolatedString(pieces) => {
                let mut typed_pieces = Vec::new();
                for piece in pieces {
                    let typed_piece = self.check_expr(piece);
                    match typed_piece.ty {
                        Type::Int | Type::Float | Type::String | Type::Boolean | Type::Error => {}
                        _ => {
                            self.error(piece.span, DiagnosticCode::TypeMismatch, &format!("Cannot interpolate type '{}'. Only Int, Float, String, and Boolean are supported.", typed_piece.ty));
                        }
                    }
                    typed_pieces.push(typed_piece);
                }
                (TypedExprKind::InterpolatedString(typed_pieces), Type::String)
            }
            ExprKind::Boolean(v) => (TypedExprKind::Boolean(*v), Type::Boolean),
            ExprKind::Null => (TypedExprKind::Null, Type::Null),
            ExprKind::Variable(name) => {
                if let Some(ty) = self.env.resolve(self.session.interner.lookup(*name)) {
                    (TypedExprKind::Variable(name.clone()), ty)
                } else if let Some(generic_stmt) = self.generic_registry.get_class(self.session.interner.lookup(*name)).cloned() {
                    if let ast::StmtKind::Class { type_params, .. } = &generic_stmt.kind {
                        (TypedExprKind::Variable(name.clone()), Type::Class(self.session.interner.lookup(*name).to_string(), type_params.iter().map(|s| self.session.interner.lookup(*s).to_string()).collect()))
                    } else {
                        unreachable!()
                    }
                } else if let Some(generic_stmt) = self.generic_registry.get_function(self.session.interner.lookup(*name)).cloned() {
                    if let ast::StmtKind::Func { type_params, params, return_type, .. } = &generic_stmt.kind {
                        self.env.push_scope();
                        for tp in type_params {
                            self.env.declare(self.session.interner.lookup(*tp).to_string(), Type::Generic(self.session.interner.lookup(*tp).to_string()));
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
                        
                        (TypedExprKind::Variable(*name), Type::Function(type_params.iter().map(|s| self.session.interner.lookup(*s).to_string()).collect(), param_types, Box::new(ret_ty)))
                    } else {
                        unreachable!()
                    }
                } else {
                    self.error(expr.span, DiagnosticCode::UnknownIdentifier, &format!("Variable \'{}\' not found.", self.session.interner.lookup(*name)));
                    (TypedExprKind::Variable(name.clone()), Type::Error)
                }
            }
            ExprKind::Assign { name, value } => {
                let typed_val = self.check_expr(value);
                if let Some(var_type) = self.env.resolve(self.session.interner.lookup(*name)) {
                    if !self.env.is_mutable(self.session.interner.lookup(*name)) {
                        self.error(expr.span, DiagnosticCode::ImmutableAssignment, &format!("Cannot mutate immutable variable \'{}\'.", self.session.interner.lookup(*name)))
                    }
                    if typed_val.ty != var_type && typed_val.ty != Type::Error && var_type != Type::Error && var_type != Type::Any {
                        self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot assign type '{}' to variable of type '{}'.", typed_val.ty, var_type))
                    }
                } else {
                    self.error(expr.span, DiagnosticCode::UnknownIdentifier, &format!("Variable \'{}\' not found.", self.session.interner.lookup(*name)))
                }
                (TypedExprKind::Assign { name: name.clone(), value: Box::new(typed_val.clone()) }, typed_val.ty)
            }
            ExprKind::SelfRef => {
                if let Some(ty) = self.env.resolve("self") {
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
                                if let Some(field_ty) = fields.get(self.session.interner.lookup(*name)) {
                                    Type::Optional(Box::new(field_ty.clone()))
                                } else {
                                    self.error(expr.span, DiagnosticCode::UnknownIdentifier, &format!("Property \'{}\' not found on '{}'.", self.session.interner.lookup(*name), class_name));
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
            ExprKind::NullCoalesce { left, right } => {
                let typed_left = self.check_expr(left);
                let typed_right = self.check_expr(right);
                
                match &typed_left.ty {
                    Type::Optional(inner) => {
                        let expected = (**inner).clone();
                        let is_valid = typed_right.ty == expected || typed_right.ty == typed_left.ty || typed_right.ty == Type::Error || typed_left.ty == Type::Error;
                        
                        if !is_valid {
                            self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot coalesce type '{}' with '{}'.", typed_left.ty, typed_right.ty));
                        }
                        
                        (TypedExprKind::NullCoalesce { left: Box::new(typed_left), right: Box::new(typed_right.clone()) }, typed_right.ty)
                    }
                    Type::Null => {
                        (TypedExprKind::NullCoalesce { left: Box::new(typed_left), right: Box::new(typed_right.clone()) }, typed_right.ty)
                    }
                    Type::Error | Type::Any => {
                        (TypedExprKind::NullCoalesce { left: Box::new(typed_left), right: Box::new(typed_right.clone()) }, typed_right.ty)
                    }
                    _ => {
                        self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Left operand of '??' must be an optional type, found '{}'.", typed_left.ty));
                        (TypedExprKind::NullCoalesce { left: Box::new(typed_left), right: Box::new(typed_right.clone()) }, typed_right.ty)
                    }
                }
            }
            ExprKind::NullCoalesceAssign { left, right } => {
                let typed_left = self.check_expr(left);
                let typed_right = self.check_expr(right);
                
                match &typed_left.ty {
                    Type::Optional(inner) => {
                        let expected = (**inner).clone();
                        if let TypedExprKind::Variable(ref left_name) = typed_left.kind {
                            if !self.env.is_mutable(self.session.interner.lookup(*left_name)) {
                                self.error(expr.span, DiagnosticCode::ImmutableAssignment, &format!("Cannot mutate immutable variable \'{}\'.", self.session.interner.lookup(*left_name)))
                            }
                        }
                        
                        let is_valid = typed_right.ty == expected || typed_right.ty == typed_left.ty || typed_right.ty == Type::Error || typed_left.ty == Type::Error;
                        
                        if !is_valid {
                            self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot assign type '{}' to variable of type '{}'.", typed_right.ty, typed_left.ty));
                        }
                        
                        (TypedExprKind::NullCoalesceAssign { left: Box::new(typed_left.clone()), right: Box::new(typed_right) }, typed_left.ty)
                    }
                    Type::Error | Type::Any => {
                        (TypedExprKind::NullCoalesceAssign { left: Box::new(typed_left.clone()), right: Box::new(typed_right) }, typed_left.ty)
                    }
                    _ => {
                        self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Left operand of '??=' must be an optional type, found '{}'.", typed_left.ty));
                        (TypedExprKind::NullCoalesceAssign { left: Box::new(typed_left.clone()), right: Box::new(typed_right) }, typed_left.ty)
                    }
                }
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
            ExprKind::ListComprehension { expr: mapped_expr, item_name, iterator } => {
                let typed_iterator = self.check_expr(iterator);
                
                let item_type = match &typed_iterator.ty {
                    Type::Range => Type::Int,
                    Type::Array(inner) => *inner.clone(),
                    Type::Error => Type::Error,
                    _ => {
                        self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot iterate over non-iterable type '{}'.", typed_iterator.ty));
                        Type::Error
                    }
                };

                self.env.push_scope();
                self.env.declare_var(self.session.interner.lookup(*item_name).to_string(), item_type, false);
                let typed_expr = self.check_expr(mapped_expr);
                self.env.pop_scope();

                let ty = Type::Array(Box::new(typed_expr.ty.clone()));
                (TypedExprKind::ListComprehension { expr: Box::new(typed_expr), item_name: item_name.clone(), iterator: Box::new(typed_iterator) }, ty)
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
                        if !self.is_assignable(&typed_val.ty, inner) && typed_val.ty != Type::Error && **inner != Type::Error && **inner != Type::Any {
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
                            self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot get property \'{}\' on non-instance type '{}'.", self.session.interner.lookup(*name), typed_obj.ty))
                        }
                        return TypedExpr::new(TypedExprKind::Get { object: Box::new(typed_obj), name: name.clone() }, Type::Error, expr.span);
                    }
                };
                
                let ty = if let Some(class_props) = self.classes.get(&class_name) {
                    if let Some(prop_ty) = class_props.get(self.session.interner.lookup(*name)) {
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
                        self.error(expr.span, DiagnosticCode::UnknownType, &format!("Property \'{}\' not found on class '{}'.", self.session.interner.lookup(*name), class_name));
                        Type::Error
                    }
                } else if let Some(interface_props) = self.interfaces.get(&class_name) {
                    if let Some(prop_ty) = interface_props.get(self.session.interner.lookup(*name)) {
                        prop_ty.clone()
                    } else {
                        self.error(expr.span, DiagnosticCode::UnknownType, &format!("Property \'{}\' not found on interface '{}'.", self.session.interner.lookup(*name), class_name));
                        Type::Error
                    }
                } else if let Some(enum_variants) = self.enums.get(&class_name) {
                    if let Some(variant_ty) = enum_variants.get(self.session.interner.lookup(*name)) {
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

                        // If it's a unit variant (no params), it evaluates to the enum type directly
                        if let Type::EnumVariantConstructor(_, _, _, params, ret_ty) = &resolved_ty
                            && params.is_empty() {
                                resolved_ty = *ret_ty.clone();
                            }

                        return TypedExpr::new(TypedExprKind::EnumVariant {
                            enum_name: self.session.interner.intern(&class_name),
                            variant_name: name.clone(),
                        }, resolved_ty, expr.span);
                    } else {
                        self.error(expr.span, DiagnosticCode::UnknownType, &format!("Variant \'{}\' not found in enum '{}'.", self.session.interner.lookup(*name), class_name));
                        Type::Error
                    }
                } else {
                    self.error(expr.span, DiagnosticCode::UnknownType, &format!("Type \'{}\' not found.", class_name));
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
                            self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot set property \'{}\' on non-instance type '{}'.", self.session.interner.lookup(*name), typed_obj.ty))
                        }
                        return TypedExpr::new(TypedExprKind::Set { object: Box::new(typed_obj), name: name.clone(), value: Box::new(typed_val.clone()) }, typed_val.ty, expr.span);
                    }
                };

                if let Some(class_props) = self.classes.get(&class_name) {
                    if let Some(prop_ty) = class_props.get(self.session.interner.lookup(*name)) {
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
                        
                        if let Some(muts) = self.class_mutables.get(&class_name) {
                            if let Some(&is_mut) = muts.get(self.session.interner.lookup(*name)) {
                                if !is_mut {
                                    self.error(expr.span, DiagnosticCode::ImmutableAssignment, &format!("Cannot mutate immutable property \'{}\'.", self.session.interner.lookup(*name)))
                                }
                            }
                        }
                        if !self.is_assignable(&typed_val.ty, &resolved_ty) && typed_val.ty != Type::Error && resolved_ty != Type::Error && resolved_ty != Type::Any {
                            self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot assign type '{}' to property of type '{}'.", typed_val.ty, resolved_ty))
                        }
                    } else {
                        self.error(expr.span, DiagnosticCode::UnknownType, &format!("Property \'{}\' not found on class '{}'.", self.session.interner.lookup(*name), class_name))
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
                                
                                if let Some(enum_name) = enum_name_opt
                                    && let Some(variants) = self.enums.get(&enum_name) {
                                        let variant_name = path.last().copied().unwrap_or_else(|| self.session.interner.intern(""));
                                        if let Some(Type::EnumVariantConstructor(_, _, func_type_params, param_types, _)) = variants.get(self.session.interner.lookup(variant_name)) {
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
                                
                                for (i, bind) in binds.iter().enumerate() {
                                    if self.session.interner.lookup(*bind) != "_" {
                                        let bind_ty = extracted_types.get(i).cloned().unwrap_or(Type::Any);
                                        self.env.declare_var(self.session.interner.lookup(*bind).to_string(), bind_ty, false);
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
                let mut typed_callee = self.check_expr(callee);

                let mut expected_param_types = None;
                match &typed_callee.ty {
                    Type::Function(_, param_types, _) => {
                        expected_param_types = Some(param_types.clone());
                    }
                    Type::OverloadedFunction(_variants) => {
                        // For overloaded functions, we'll try to infer based on passed arguments below.
                        // We don't have a single expected param list.
                    }
                    Type::EnumVariantConstructor(_, _, _, param_types, _) => {
                        expected_param_types = Some(param_types.clone());
                    }
                    Type::Class(class_name, _) => {
                        if let Some(props) = self.classes.get(class_name)
                            && let Some(Type::Function(_, param_types, _)) = props.get("init") {
                                expected_param_types = Some(param_types.clone());
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
                    Type::OverloadedFunction(variants) => {
                        let mut matched_variant = None;
                        for (mangled_name, ty) in variants {
                            if let Type::Function(_, param_types, ret_ty) = ty
                                && param_types.len() == arg_types.len() {
                                    let mut matches = true;
                                    for (pt, at) in param_types.iter().zip(arg_types.iter()) {
                                        if !self.is_assignable(at, pt) {
                                            matches = false;
                                            break;
                                        }
                                    }
                                    if matches {
                                        matched_variant = Some((mangled_name.clone(), ty.clone(), ret_ty.clone()));
                                        break;
                                    }
                                }
                        }

                        if let Some((mangled_name, ty, ret_ty)) = matched_variant {
                            typed_callee = TypedExpr {
                                kind: TypedExprKind::Variable(self.session.interner.intern(&mangled_name)),
                                ty,
                                span: typed_callee.span,
                            };
                            *ret_ty
                        } else {
                            self.error(expr.span, DiagnosticCode::TypeMismatch, "No matching overload found for arguments.");
                            Type::Error
                        }
                    }
                    Type::Class(class_name, class_type_params) => {
                        let mut constructor_ty = self.classes.get(class_name)
                            .and_then(|props| props.get("init").cloned());
                            
                        if constructor_ty.is_none()
                            && let Some(generic_stmt) = self.generic_registry.get_class(class_name).cloned()
                                && let ast::StmtKind::Class { type_params, methods, .. } = &generic_stmt.kind {
                                    self.env.push_scope();
                                    for tp in type_params {
                                        self.env.declare(self.session.interner.lookup(*tp).to_string(), Type::Generic(self.session.interner.lookup(*tp).to_string()));
                                    }
                                    for method in methods {
                                        if let ast::StmtKind::Func { name, params, .. } = &method.kind
                                            && self.session.interner.lookup(*name) == "init" {
                                                let mut param_types = Vec::new();
                                                for (_, ty) in params {
                                                    param_types.push(self.parse_type(ty, expr.span));
                                                }

                                                constructor_ty = Some(Type::Function(Vec::new(), param_types, Box::new(Type::Void)));
                                            }
                                    }
                                    self.env.pop_scope();
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
                                        kind: TypedExprKind::Variable(self.session.interner.intern(&mangled_name)),
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
                            self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Class \'{}\' has no 'init' method but arguments were provided.", class_name))
                        }
                        
                        if !class_type_params.is_empty() {
                            let mangled_name = self.instantiate_generic_class(class_name, class_type_params, &resolved_type_args);
                            
                            let new_callee = TypedExpr {
                                kind: TypedExprKind::Variable(self.session.interner.intern(&mangled_name)),
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
                                let func_name_str = self.session.interner.lookup(*func_name).to_string();
                                let mangled = self.instantiate_generic_function(&func_name_str, func_type_params, &resolved_type_args);
                                
                                let new_callee = TypedExpr {
                                    kind: TypedExprKind::Variable(self.session.interner.intern(&mangled)),
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
                                        enum_name: self.session.interner.intern(&enum_name),
                                        variant_name: self.session.interner.intern(&variant_name),
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
            ExprKind::Range { start, end } => {
                let typed_start = self.check_expr_with_expected(start, Some(&Type::Int));
                let typed_end = self.check_expr_with_expected(end, Some(&Type::Int));
                
                if typed_start.ty != Type::Int || typed_end.ty != Type::Int {
                    self.error(expr.span, DiagnosticCode::TypeMismatch, "Range bounds must be integers.");
                }
                
                (TypedExprKind::Range { start: Box::new(typed_start), end: Box::new(typed_end) }, Type::Range)
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
            TypeExpr::Named(name) => match self.session.interner.lookup(*name) {
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
                    if let Some(Type::Generic(g)) = self.env.resolve(self.session.interner.lookup(*name)) {
                        return Type::Generic(g.clone());
                    }
                    if self.classes.contains_key(self.session.interner.lookup(*name)) || self.enums.contains_key(self.session.interner.lookup(*name)) {
                        Type::Instance(self.session.interner.lookup(*name).to_string())
                    } else if self.interfaces.contains_key(self.session.interner.lookup(*name)) {
                        Type::Interface(self.session.interner.lookup(*name).to_string())
                    } else {
                        self.error(span, DiagnosticCode::UnknownType, &format!("Unknown type \'{}\'.", self.session.interner.lookup(*name)));

                        Type::Error
                    }
                }
            },
            TypeExpr::GenericInstance(name, args) => {
                let parsed_args = args.iter().map(|a| self.parse_type(a, span)).collect::<Vec<_>>();
                if self.session.interner.lookup(*name) == "Pointer" && parsed_args.len() == 1 {
                    Type::Pointer(Box::new(parsed_args[0].clone()))
                } else if self.classes.contains_key(self.session.interner.lookup(*name)) || self.enums.contains_key(self.session.interner.lookup(*name)) {
                    Type::GenericInstance(self.session.interner.lookup(*name).to_string(), parsed_args)
                } else {
                    self.error(span, DiagnosticCode::UnknownType, &format!("Unknown generic class \'{}\'.", self.session.interner.lookup(*name)));

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
        if source == target || *source == Type::Error || *target == Type::Error {
            return true;
        }
        
        let is_source_int = matches!(source, Type::Int | Type::CInt | Type::CUInt | Type::CChar | Type::CSize);
        let is_target_int = matches!(target, Type::Int | Type::CInt | Type::CUInt | Type::CChar | Type::CSize);
        if is_source_int && is_target_int {
            return true;
        }
        if *source == Type::Null
            && matches!(target, Type::Optional(_)) {
                return true;
            }
        if let Type::Optional(inner) = target
            && self.is_assignable(source, inner) {
                return true;
            }
        if *target == Type::Any {
            return true;
        }
        if let (Type::Instance(class_name), Type::Interface(interface_name)) = (source, target)
            && let Some(implements) = self.class_implements.get(class_name)
                && implements.contains(interface_name) {
                    return true;
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

    pub fn error(&mut self, span: Span, code: DiagnosticCode, message: &str) {
        self.errors.push(DiagnosticBuilder::error(code, message, span).build());
    }

    fn type_to_type_expr(&mut self, ty: &Type) -> ast::TypeExpr {
        match ty {
            Type::Int => ast::TypeExpr::Named(self.session.interner.intern("Int")),
            Type::Float => ast::TypeExpr::Named(self.session.interner.intern("Float")),
            Type::Boolean => ast::TypeExpr::Named(self.session.interner.intern("Boolean")),
            Type::String => ast::TypeExpr::Named(self.session.interner.intern("String")),
            Type::Instance(name) | Type::Interface(name) => ast::TypeExpr::Named(self.session.interner.intern(name)),
            Type::GenericInstance(name, args) => {
                ast::TypeExpr::GenericInstance(self.session.interner.intern(name), args.iter().map(|t| self.type_to_type_expr(t)).collect())
            }
            Type::Optional(inner) => ast::TypeExpr::Optional(Box::new(self.type_to_type_expr(inner))),
            Type::Array(inner) => ast::TypeExpr::Array(Box::new(self.type_to_type_expr(inner))),
            _ => ast::TypeExpr::Named(self.session.interner.intern("Any")),
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
                type_arg_exprs.push(self.type_to_type_expr(ty));
            }

            let param_syms: Vec<session::Symbol> = type_params.iter().map(|s| self.session.interner.intern(s)).collect();
            let substitution = generics::TypeSubstitution::new(&param_syms, &type_arg_exprs);
            let monomorphizer = generics::Monomorphizer::new(&substitution, self.session.interner.intern(&mangled_name));
            
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
                type_arg_exprs.push(self.type_to_type_expr(ty));
            }

            let param_syms: Vec<session::Symbol> = type_params.iter().map(|s| self.session.interner.intern(s)).collect();
            let substitution = generics::TypeSubstitution::new(&param_syms, &type_arg_exprs);
            let monomorphizer = generics::Monomorphizer::new(&substitution, self.session.interner.intern(&mangled_name));
            
            let mut concrete_stmt = monomorphizer.monomorphize_stmt(&generic_stmt);
            if let ast::StmtKind::Func { name, .. } = &mut concrete_stmt.kind {
                *name = self.session.interner.intern(&mangled_name);
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
    fn get_assigned_properties_in_init(stmt: &TypedStmt) -> std::collections::HashSet<session::Symbol> {
        let mut assigned = std::collections::HashSet::new();
        match &stmt.kind {
            TypedStmtKind::Block(stmts) => {
                for s in stmts {
                    assigned.extend(Self::get_assigned_properties_in_init(s));
                }
            }
            TypedStmtKind::Expression(expr) => {
                if let TypedExprKind::Set { object, name, value: _ } = &expr.kind
                    && let TypedExprKind::SelfRef = &object.kind {
                        assigned.insert(*name);
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
            TypedStmtKind::While { .. } | TypedStmtKind::For { .. } => {
                // Loop bodies might not execute, so we don't count assignments inside them as definite!
            }
            _ => {}
        }
        assigned
    }
}

#[cfg(any())]
mod tests {
    use super::*;
    use ast::Location;

    fn make_span() -> Span {
        Span::new(0, 0, 0, Location::new(1, 1), Location::new(1, 1))
    }

    #[test]
    fn test_valid_math() {
        let mut session = session::CompilerSession::new();
        let mut checker = TypeChecker::new(&mut session);
        // let x = 10 + 5;
        let stmt = Stmt::new(StmtKind::Let {
            name: session.interner.intern("x"),
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
        let mut session = session::CompilerSession::new();
        let mut checker = TypeChecker::new(&mut session);
        // let x = 10 + "hello";
        let stmt = Stmt::new(StmtKind::Let {
            name: session.interner.intern("x"),
            is_private: false,
            type_annotation: None,
            initializer: Some(Expr::new(ExprKind::Binary(
                Box::new(Expr::new(ExprKind::Integer(10), make_span())),
                BinaryOp::Add,
                Box::new(Expr::new(ExprKind::String(session.interner.intern("hello")), make_span())),
            ), make_span())),
        }, make_span());

        checker.check(&[stmt]);
        assert_eq!(checker.errors.len(), 1);
        assert!(checker.errors[0].message.contains("Cannot apply operator to types 'Int' and 'String'"));
    }

    #[test]
    fn test_if_condition_type() {
        let mut session = session::CompilerSession::new();
        let mut checker = TypeChecker::new(&mut session);
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

    #[test]
    fn test_immutable_assignment() {
        let mut session = session::CompilerSession::new();
        let mut checker = TypeChecker::new(&mut session);
        // let x = 10;
        let stmt1 = Stmt::new(StmtKind::Let {
            name: session.interner.intern("x"),
            is_private: false,
            type_annotation: None,
            initializer: Some(Expr::new(ExprKind::Integer(10), make_span())),
        }, make_span());

        // x = 20;
        let stmt2 = Stmt::new(StmtKind::Expression(
            Expr::new(ExprKind::Assign {
                name: session.interner.intern("x"),
                value: Box::new(Expr::new(ExprKind::Integer(20), make_span())),
            }, make_span())
        ), make_span());

        checker.check(&[stmt1, stmt2]);
        assert_eq!(checker.errors.len(), 1);
        assert!(checker.errors[0].message.contains("Cannot mutate immutable variable 'x'"));
    }
}
