use ast::{Expr, ExprKind, Stmt, StmtKind, Span, BinaryOp, UnaryOp, TypeExpr};
use crate::types::Type;
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
    pub class_implements: HashMap<String, Vec<String>>,
    current_class: Option<String>,
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            env: TypeEnvironment::new(),
            errors: Vec::new(),
            current_return_type: None,
            classes: HashMap::new(),
            interfaces: HashMap::new(),
            class_implements: HashMap::new(),
            current_class: None,
        }
    }

    pub fn check(&mut self, statements: &[Stmt]) {
        for stmt in statements {
            self.check_stmt(stmt);
        }
    }

    fn check_stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Block(stmts) => {
                self.env.push_scope();
                self.check(stmts);
                self.env.pop_scope();
            }
            StmtKind::Let { name, type_annotation, initializer } => self.check_var_decl(name, type_annotation, initializer, false, stmt.span),
            StmtKind::Var { name, type_annotation, initializer, is_weak } => self.check_var_decl(name, type_annotation, initializer, *is_weak, stmt.span),
            StmtKind::Class { name, type_params, implements, methods, fields } => {
                self.env.declare(name.clone(), Type::Class(name.clone(), type_params.clone()));
                self.classes.insert(name.clone(), std::collections::HashMap::new());
                
                self.env.push_scope();
                for tp in type_params {
                    self.env.declare(tp.clone(), Type::Generic(tp.clone()));
                }

                let mut class_members = HashMap::new();
                
                for field in fields {
                    let (f_name, type_annotation, initializer, is_weak) = match &field.kind {
                        StmtKind::Var { name, type_annotation, initializer, is_weak } => (name, type_annotation, initializer, *is_weak),
                        StmtKind::Let { name, type_annotation, initializer } => (name, type_annotation, initializer, false),
                        _ => continue,
                    };
                    
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
                            if !matches!(parsed, Type::Optional(ref inner) if matches!(**inner, Type::Instance(_) | Type::Interface(_))) {
                                self.error(field.span, DiagnosticCode::TypeMismatch, "Weak properties must be of optional instance type (e.g. 'weak var x: User?').");
                            }
                        }
                        parsed
                    } else {
                        if is_weak {
                            self.error(field.span, DiagnosticCode::TypeMismatch, "Weak properties must be of optional instance type (e.g. 'weak var x: User?').");
                        }
                        Type::Any
                    };
                    class_members.insert(f_name.clone(), ty);
                }

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
                        class_members.insert(m_name.clone(), Type::Function(param_types, Box::new(ret_ty)));
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
                
                for method in methods {
                    self.check_stmt(method);
                }
                
                self.env.pop_scope();
                self.current_class = prev_class;
            }
            StmtKind::Interface { name, methods } => {
                self.env.declare(name.clone(), Type::Interface(name.clone()));
                
                let mut interface_members = HashMap::new();
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
                        interface_members.insert(m_name.clone(), Type::Function(param_types, Box::new(ret_ty)));
                    }
                }
                
                self.interfaces.insert(name.clone(), interface_members);
            }
            StmtKind::Func { name, type_params, params, return_type, body } => {
                self.env.push_scope();
                for tp in type_params {
                    self.env.declare(tp.clone(), Type::Generic(tp.clone()));
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

                self.env.declare(name.clone(), Type::Function(param_types.clone(), Box::new(ret_ty.clone())));

                
                if let Some(ref class_name) = self.current_class {
                    self.env.declare("self".to_string(), Type::Instance(class_name.clone()));
                }

                for ((param_name, _), param_ty) in params.iter().zip(param_types.into_iter()) {
                    self.env.declare(param_name.clone(), param_ty);
                }

                let previous_return = self.current_return_type.take();
                self.current_return_type = Some(ret_ty.clone());

                self.check_stmt(body);

                self.current_return_type = previous_return;
                self.env.pop_scope();
            }
            StmtKind::If { condition, then_branch, else_branch } => {
                let cond_type = self.check_expr(condition);
                if cond_type != Type::Boolean && cond_type != Type::Error {
                    self.error(condition.span, DiagnosticCode::TypeMismatch, &format!("Expected 'Boolean' for if condition, found '{}'.", cond_type))
                }

                self.check_stmt(then_branch);
                if let Some(e_branch) = else_branch {
                    self.check_stmt(e_branch);
                }
            }
            StmtKind::While { condition, body } => {
                let cond_type = self.check_expr(condition);
                if cond_type != Type::Boolean && cond_type != Type::Error {
                    self.error(condition.span, DiagnosticCode::TypeMismatch, &format!("Expected 'Boolean' for while condition, found '{}'.", cond_type))
                }

                self.check_stmt(body);
            }
            StmtKind::For { item_name, iterator, body } => {
                let _iter_type = self.check_expr(iterator);
                // Basic implementation: we don't know what type is inside the iterator yet without generics/arrays.
                // We will default the item to Error so it ignores subsequent type errors inside the loop.
                self.env.push_scope();
                self.env.declare(item_name.clone(), Type::Error);
                self.check_stmt(body);
                self.env.pop_scope();
            }
            StmtKind::Expression(expr) => {
                self.check_expr(expr);
            }
            StmtKind::Return { value } => {
                let value_type = if let Some(val) = value {
                    self.check_expr(val)
                } else {
                    Type::Void
                };

                if let Some(expected) = &self.current_return_type {
                    if *expected != value_type && value_type != Type::Error && *expected != Type::Error {
                        self.error(stmt.span, DiagnosticCode::TypeMismatch, &format!("Cannot return value of type '{}' from function expecting '{}'.", value_type, expected))
                    }
                } else {
                    self.error(stmt.span, DiagnosticCode::TypeMismatch, "Cannot return from outside a function.")
                }
            }
        }
    }

    fn check_var_decl(&mut self, name: &String, type_annotation: &Option<TypeExpr>, initializer: &Option<Expr>, is_weak: bool, span: Span) {
        let mut init_type = if let Some(init) = initializer {
            self.check_expr(init)
        } else {
            Type::Any
        };
        
        if let Some(ann) = type_annotation {
            let ann_type = self.parse_type(ann, span);
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
    }

    fn check_expr(&mut self, expr: &Expr) -> Type {
        match &expr.kind {
            ExprKind::Integer(_) => Type::Int,
            ExprKind::Float(_) => Type::Float,
            ExprKind::String(_) => Type::String,
            ExprKind::Boolean(_) => Type::Boolean,
            ExprKind::Null => Type::Null,
            ExprKind::Variable(name) => {
                if let Some(ty) = self.env.resolve(name) {
                    ty
                } else {
                    Type::Error
                }
            }
            ExprKind::Assign { name, value } => {
                let val_type = self.check_expr(value);
                if let Some(var_type) = self.env.resolve(name) {
                    if val_type != var_type && val_type != Type::Error && var_type != Type::Error && var_type != Type::Any {
                        self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot assign type '{}' to variable of type '{}'.", val_type, var_type))
                    }
                } else {
                    self.error(expr.span, DiagnosticCode::UnknownIdentifier, &format!("Variable '{}' not found.", name))
                }
                val_type
            }
            ExprKind::SelfRef => {
                if let Some(ty) = self.env.resolve(&"self".to_string()) {
                    ty
                } else {
                    self.error(expr.span, DiagnosticCode::TypeMismatch, "Cannot use 'self' outside a class.");

                    Type::Error
                }
            }
            ExprKind::ForceUnwrap(inner) => {
                let inner_ty = self.check_expr(inner);
                match inner_ty {
                    Type::Optional(inner_inner) => *inner_inner,
                    Type::Null => {
                        self.error(expr.span, DiagnosticCode::TypeMismatch, "Cannot force unwrap a null literal.");
                        Type::Error
                    }
                    Type::Error | Type::Any => inner_ty,
                    _ => {
                        self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot force unwrap non-optional type '{}'.", inner_ty));
                        inner_ty
                    }
                }
            }
            ExprKind::OptionalGet { object, name } => {
                let obj_ty = self.check_expr(object);
                match obj_ty {
                    Type::Optional(inner) => {
                        if let Type::Instance(class_name) = &*inner {
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
                    Type::Error | Type::Any => obj_ty,
                    _ => {
                        self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Optional chaining '?.' requires an optional type, found '{}'.", obj_ty));
                        Type::Error
                    }
                }
            }
            ExprKind::Array(elements) => {
                if elements.is_empty() {
                    self.error(expr.span, DiagnosticCode::TypeMismatch, "Cannot infer type of empty array literal.");
                    return Type::Error;
                }
                let elem_type = self.check_expr(&elements[0]);
                for elem in elements.iter().skip(1) {
                    let next_type = self.check_expr(elem);
                    if next_type != elem_type && next_type != Type::Error && elem_type != Type::Error {
                        self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Array elements have inconsistent types: expected '{}', found '{}'.", elem_type, next_type));
                    }
                }
                Type::Array(Box::new(elem_type))
            }
            ExprKind::ArrayRepeat { value, count } => {
                let elem_type = self.check_expr(value);
                let count_type = self.check_expr(count);
                if count_type != Type::Int && count_type != Type::Error {
                    self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Array repeat count must be 'Int', found '{}'.", count_type));
                }
                Type::Array(Box::new(elem_type))
            }
            ExprKind::IndexGet { object, index } => {
                let obj_type = self.check_expr(object);
                let idx_type = self.check_expr(index);
                if idx_type != Type::Int && idx_type != Type::Error {
                    self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Array index must be 'Int', found '{}'.", idx_type));
                }
                match obj_type {
                    Type::Array(inner) => *inner,
                    Type::Error => Type::Error,
                    _ => {
                        self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot index into non-array type '{}'.", obj_type));
                        Type::Error
                    }
                }
            }
            ExprKind::IndexSet { object, index, value } => {
                let obj_type = self.check_expr(object);
                let idx_type = self.check_expr(index);
                let val_type = self.check_expr(value);
                
                if idx_type != Type::Int && idx_type != Type::Error {
                    self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Array index must be 'Int', found '{}'.", idx_type));
                }
                
                match obj_type {
                    Type::Array(inner) => {
                        if !self.is_assignable(&val_type, &inner) && val_type != Type::Error && *inner != Type::Error && *inner != Type::Any {
                            self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot assign type '{}' to array element of type '{}'.", val_type, inner));
                        }
                    }
                    Type::Error => {}
                    _ => {
                        self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot index into non-array type '{}'.", obj_type));
                    }
                }
                val_type
            }
            ExprKind::Get { object, name } => {
                let obj_type = self.check_expr(object);
                
                let (class_name, instance_args) = match &obj_type {
                    Type::Instance(n) => (n.clone(), Vec::new()),
                    Type::GenericInstance(n, args) => (n.clone(), args.clone()),
                    Type::Interface(n) => (n.clone(), Vec::new()),
                    _ => {
                        if obj_type != Type::Error {
                            self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot get property '{}' on non-instance type '{}'.", name, obj_type))
                        }
                        return Type::Error;
                    }
                };
                
                if let Some(class_props) = self.classes.get(&class_name) {
                    if let Some(prop_ty) = class_props.get(name) {
                        let mut resolved_ty = prop_ty.clone();
                        if let Type::Generic(g) = prop_ty {
                            if let Some(Type::Class(_, params)) = self.env.resolve(&class_name) {
                                if let Some(idx) = params.iter().position(|p| p == g) {
                                    if idx < instance_args.len() {
                                        resolved_ty = instance_args[idx].clone();
                                    }
                                }
                            }
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
                } else {
                    Type::Error
                }
            }
            ExprKind::Set { object, name, value } => {
                let obj_type = self.check_expr(object);
                let val_type = self.check_expr(value);
                
                let (class_name, instance_args) = match &obj_type {
                    Type::Instance(n) => (n.clone(), Vec::new()),
                    Type::GenericInstance(n, args) => (n.clone(), args.clone()),
                    Type::Interface(n) => (n.clone(), Vec::new()),
                    _ => {
                        if obj_type != Type::Error {
                            self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot set property '{}' on non-instance type '{}'.", name, obj_type))
                        }
                        return val_type;
                    }
                };

                if let Some(class_props) = self.classes.get(&class_name) {
                    if let Some(prop_ty) = class_props.get(name) {
                        let mut resolved_ty = prop_ty.clone();
                        if let Type::Generic(g) = prop_ty {
                            if let Some(Type::Class(_, params)) = self.env.resolve(&class_name) {
                                if let Some(idx) = params.iter().position(|p| p == g) {
                                    if idx < instance_args.len() {
                                        resolved_ty = instance_args[idx].clone();
                                    }
                                }
                            }
                        }
                        
                        if !self.is_assignable(&val_type, &resolved_ty) && val_type != Type::Error && resolved_ty != Type::Error && resolved_ty != Type::Any {
                            self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot assign type '{}' to property of type '{}'.", val_type, resolved_ty))
                        }
                    } else {
                        self.error(expr.span, DiagnosticCode::UnknownType, &format!("Property '{}' not found on class '{}'.", name, class_name))
                    }
                }
                val_type
            }
            ExprKind::Grouping(inner) => {
                self.check_expr(inner)
            }
            ExprKind::Call { callee, type_args, arguments } => {
                let callee_type = self.check_expr(callee);
                let mut arg_types = Vec::new();
                for arg in arguments {
                    arg_types.push(self.check_expr(arg));
                }
                
                match callee_type {
                    Type::BuiltinFunc => Type::Void,
                    Type::Class(class_name, class_type_params) => {
                        let constructor_ty = self.classes.get(&class_name)
                            .and_then(|props| props.get("init").cloned());
                            
                        let mut resolved_type_args = Vec::new();

                        if let Some(Type::Function(param_types, _)) = constructor_ty {
                            if param_types.len() != arg_types.len() {
                                self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Constructor expected {} arguments, found {}.", param_types.len(), arg_types.len()))
                            } else {
                                // Basic Local Inference & Checking
                                if !class_type_params.is_empty() {
                                    if type_args.is_empty() {
                                        // Infer from arguments
                                        let mut inferred_map = std::collections::HashMap::new();
                                        for (expected, actual) in param_types.iter().zip(arg_types.iter()) {
                                            if let Type::Generic(g) = expected {
                                                if let std::collections::hash_map::Entry::Vacant(e) = inferred_map.entry(g.clone()) {
                                                    e.insert(actual.clone());
                                                }
                                            }
                                        }
                                        
                                        for tp in &class_type_params {
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
                                
                                // TODO: Substitute generic parameters when checking constructor argument types!
                                for (i, (expected, actual)) in param_types.iter().zip(arg_types.iter()).enumerate() {
                                    // simple substitution for now
                                    let mut expected_sub = expected.clone();
                                    if let Type::Generic(g) = expected {
                                        if let Some(idx) = class_type_params.iter().position(|p| p == g) {
                                            if idx < resolved_type_args.len() {
                                                expected_sub = resolved_type_args[idx].clone();
                                            }
                                        }
                                    }
                                    
                                    if !self.is_assignable(actual, &expected_sub) && expected_sub != Type::Any && *actual != Type::Error {
                                        self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Argument {} to constructor expects '{}', found '{}'.", i + 1, expected_sub, actual))
                                    }
                                }
                            }
                        } else if !arg_types.is_empty() {
                            self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Class '{}' has no 'init' method but arguments were provided.", class_name))
                        }
                        
                        if class_type_params.is_empty() {
                            Type::Instance(class_name)
                        } else {
                            Type::GenericInstance(class_name, resolved_type_args)
                        }
                    }
                    Type::Function(param_types, ret_ty) => {
                        if param_types.len() != arg_types.len() {
                            self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Expected {} arguments, found {}.", param_types.len(), arg_types.len()))
                        } else {
                            for (i, (expected, actual)) in param_types.iter().zip(arg_types.iter()).enumerate() {
                                if !self.is_assignable(actual, expected) && *expected != Type::Any && *actual != Type::Error {
                                    self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Argument {} expected type '{}', found '{}'.", i + 1, expected, actual))
                                }
                            }
                        }
                        *ret_ty
                    }
                    Type::Error => Type::Error,
                    _ => {
                        self.error(expr.span, DiagnosticCode::TypeMismatch, "Cannot call non-function type.");

                        Type::Error
                    }
                }
            }
            ExprKind::Unary(op, right) => {
                let right_type = self.check_expr(right);
                if right_type == Type::Error {
                    return Type::Error;
                }

                match op {
                    UnaryOp::Negate => {
                        if right_type == Type::Int || right_type == Type::Float {
                            right_type
                        } else {
                            self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot negate type '{}'.", right_type));

                            Type::Error
                        }
                    }
                }
            }
            ExprKind::Binary(left, op, right) => {
                let left_type = self.check_expr(left);
                let right_type = self.check_expr(right);

                if left_type == Type::Error || right_type == Type::Error {
                    return Type::Error;
                }

                match op {
                    BinaryOp::Add | BinaryOp::Subtract | BinaryOp::Multiply | BinaryOp::Divide => {
                        if left_type == right_type && (left_type == Type::Int || left_type == Type::Float) {
                            left_type
                        } else if *op == BinaryOp::Add && left_type == Type::String && right_type == Type::String {
                            Type::String
                        } else {
                            self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot apply operator to types '{}' and '{}'.", left_type, right_type));
                            Type::Error
                        }
                    }
                    BinaryOp::Equal | BinaryOp::NotEqual => {
                        if left_type != right_type {
                            self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot compare types '{}' and '{}' for equality.", left_type, right_type));
                            Type::Error
                        } else {
                            Type::Boolean
                        }
                    }
                    BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual => {
                        if left_type == right_type && (left_type == Type::Int || left_type == Type::Float) {
                            Type::Boolean
                        } else {
                            self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot apply comparison to types '{}' and '{}'.", left_type, right_type));

                            Type::Error
                        }
                    }
                }
            }
        }
    }

    fn parse_type(&mut self, type_expr: &TypeExpr, span: Span) -> Type {
        match type_expr {
            TypeExpr::Named(name) => match name.as_str() {
                "Int" => Type::Int,
                "Float" => Type::Float,
                "String" => Type::String,
                "Boolean" => Type::Boolean,
                "Void" => Type::Void,
                _ => {
                    if let Some(Type::Generic(g)) = self.env.resolve(name) {
                        return Type::Generic(g.clone());
                    }
                    if self.classes.contains_key(name) {
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
                let parsed_args = args.iter().map(|a| self.parse_type(a, span)).collect();
                if self.classes.contains_key(name) {
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

    fn error(&mut self, span: Span, code: DiagnosticCode, message: &str) {
        self.errors.push(DiagnosticBuilder::error(code, message, span).build());
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
