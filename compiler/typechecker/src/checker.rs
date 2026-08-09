use ast::{Expr, ExprKind, Stmt, StmtKind, Span, BinaryOp, UnaryOp};
use crate::types::Type;
use crate::env::TypeEnvironment;
use std::collections::HashMap;

#[derive(Debug)]
pub struct TypeError {
    pub message: String,
    pub span: Span,
}

pub struct TypeChecker {
    env: TypeEnvironment,
    pub errors: Vec<TypeError>,
    current_return_type: Option<Type>,
    pub classes: HashMap<String, HashMap<String, Type>>,
    current_class: Option<String>,
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            env: TypeEnvironment::new(),
            errors: Vec::new(),
            current_return_type: None,
            classes: HashMap::new(),
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
            StmtKind::Let { name, type_annotation, initializer } | StmtKind::Var { name, type_annotation, initializer } => {
                let mut init_type = if let Some(init) = initializer {
                    self.check_expr(init)
                } else {
                    Type::Any
                };
                
                if let Some(ann) = type_annotation {
                    let ann_type = self.parse_type(ann, stmt.span);
                    if init_type == Type::Any {
                        init_type = ann_type;
                    } else if init_type != ann_type && init_type != Type::Error {
                        self.error(stmt.span, &format!("Cannot assign type '{}' to variable of type '{}'.", init_type, ann_type));
                    }
                }
                
                self.env.declare(name.clone(), init_type);
            }
            StmtKind::Class { name, methods, fields } => {
                self.env.declare(name.clone(), Type::Class(name.clone()));
                
                let mut class_members = HashMap::new();
                
                for field in fields {
                    if let StmtKind::Var { name: f_name, type_annotation, initializer } | StmtKind::Let { name: f_name, type_annotation, initializer } = &field.kind {
                        let ty = if let Some(ann) = type_annotation {
                            self.parse_type(ann, field.span)
                        } else if let Some(init) = initializer {
                            self.check_expr(init)
                        } else {
                            Type::Any
                        };
                        class_members.insert(f_name.clone(), ty);
                    }
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
                
                self.classes.insert(name.clone(), class_members);
                
                let prev_class = self.current_class.clone();
                self.current_class = Some(name.clone());
                
                for method in methods {
                    self.check_stmt(method);
                }
                
                self.current_class = prev_class;
            }
            StmtKind::Func { name, params, return_type, body } => {
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

                self.env.push_scope();
                
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
                    self.error(condition.span, &format!("Expected 'Boolean' for if condition, found '{}'.", cond_type));
                }

                self.check_stmt(then_branch);
                if let Some(e_branch) = else_branch {
                    self.check_stmt(e_branch);
                }
            }
            StmtKind::While { condition, body } => {
                let cond_type = self.check_expr(condition);
                if cond_type != Type::Boolean && cond_type != Type::Error {
                    self.error(condition.span, &format!("Expected 'Boolean' for while condition, found '{}'.", cond_type));
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
                        self.error(stmt.span, &format!("Cannot return value of type '{}' from function expecting '{}'.", value_type, expected));
                    }
                } else {
                    self.error(stmt.span, "Cannot return from outside a function.");
                }
            }
        }
    }

    fn check_expr(&mut self, expr: &Expr) -> Type {
        match &expr.kind {
            ExprKind::Integer(_) => Type::Int,
            ExprKind::Float(_) => Type::Float,
            ExprKind::String(_) => Type::String,
            ExprKind::Boolean(_) => Type::Boolean,
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
                        self.error(expr.span, &format!("Cannot assign type '{}' to variable of type '{}'.", val_type, var_type));
                    }
                } else {
                    self.error(expr.span, &format!("Variable '{}' not found.", name));
                }
                val_type
            }
            ExprKind::SelfRef => {
                if let Some(ty) = self.env.resolve(&"self".to_string()) {
                    ty
                } else {
                    self.error(expr.span, "Cannot use 'self' outside a class.");
                    Type::Error
                }
            }
            ExprKind::Get { object, name } => {
                let obj_type = self.check_expr(object);
                if let Type::Instance(class_name) = obj_type {
                    if let Some(class_props) = self.classes.get(&class_name) {
                        if let Some(prop_ty) = class_props.get(name) {
                            prop_ty.clone()
                        } else {
                            self.error(expr.span, &format!("Property '{}' not found on class '{}'.", name, class_name));
                            Type::Error
                        }
                    } else {
                        Type::Error
                    }
                } else if obj_type == Type::Error {
                    Type::Error
                } else {
                    self.error(expr.span, &format!("Cannot get property '{}' on non-instance type '{}'.", name, obj_type));
                    Type::Error
                }
            }
            ExprKind::Set { object, name, value } => {
                let obj_type = self.check_expr(object);
                let val_type = self.check_expr(value);
                
                if let Type::Instance(class_name) = obj_type {
                    if let Some(class_props) = self.classes.get(&class_name) {
                        if let Some(prop_ty) = class_props.get(name) {
                            if val_type != *prop_ty && val_type != Type::Error && *prop_ty != Type::Error && *prop_ty != Type::Any {
                                self.error(expr.span, &format!("Cannot assign type '{}' to property of type '{}'.", val_type, prop_ty));
                            }
                        } else {
                            self.error(expr.span, &format!("Property '{}' not found on class '{}'.", name, class_name));
                        }
                    }
                } else if obj_type != Type::Error {
                    self.error(expr.span, &format!("Cannot set property '{}' on non-instance type '{}'.", name, obj_type));
                }
                val_type
            }
            ExprKind::Grouping(inner) => {
                self.check_expr(inner)
            }
            ExprKind::Call { callee, arguments } => {
                let callee_type = self.check_expr(callee);
                let mut arg_types = Vec::new();
                for arg in arguments {
                    arg_types.push(self.check_expr(arg));
                }
                
                match callee_type {
                    Type::BuiltinFunc => Type::Void,
                    Type::Class(class_name) => {
                        let constructor_ty = self.classes.get(&class_name)
                            .and_then(|props| props.get("init").cloned());
                        
                        if let Some(Type::Function(param_types, _)) = constructor_ty {
                            if param_types.len() != arg_types.len() {
                                self.error(expr.span, &format!("Constructor expected {} arguments, found {}.", param_types.len(), arg_types.len()));
                            } else {
                                for (i, (expected, actual)) in param_types.iter().zip(arg_types.iter()).enumerate() {
                                    if expected != actual && *expected != Type::Any && *actual != Type::Error {
                                        self.error(expr.span, &format!("Argument {} expected type '{}', found '{}'.", i + 1, expected, actual));
                                    }
                                }
                            }
                        } else if !arg_types.is_empty() {
                            self.error(expr.span, &format!("Class '{}' has no init method, expected 0 arguments.", class_name));
                        }
                        Type::Instance(class_name)
                    }
                    Type::Function(param_types, ret_ty) => {
                        if param_types.len() != arg_types.len() {
                            self.error(expr.span, &format!("Expected {} arguments, found {}.", param_types.len(), arg_types.len()));
                        } else {
                            for (i, (expected, actual)) in param_types.iter().zip(arg_types.iter()).enumerate() {
                                if expected != actual && *expected != Type::Any && *actual != Type::Error {
                                    self.error(expr.span, &format!("Argument {} expected type '{}', found '{}'.", i + 1, expected, actual));
                                }
                            }
                        }
                        *ret_ty
                    }
                    Type::Error => Type::Error,
                    _ => {
                        self.error(expr.span, "Cannot call non-function type.");
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
                            self.error(expr.span, &format!("Cannot negate type '{}'.", right_type));
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
                            self.error(expr.span, &format!("Cannot apply operator to types '{}' and '{}'.", left_type, right_type));
                            Type::Error
                        }
                    }
                    BinaryOp::Equal | BinaryOp::NotEqual => {
                        if left_type != right_type {
                            self.error(expr.span, &format!("Cannot compare types '{}' and '{}' for equality.", left_type, right_type));
                            Type::Error
                        } else {
                            Type::Boolean
                        }
                    }
                    BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual => {
                        if left_type == right_type && (left_type == Type::Int || left_type == Type::Float) {
                            Type::Boolean
                        } else {
                            self.error(expr.span, &format!("Cannot apply comparison to types '{}' and '{}'.", left_type, right_type));
                            Type::Error
                        }
                    }
                }
            }
        }
    }

    fn parse_type(&mut self, name: &str, span: Span) -> Type {
        match name {
            "Int" => Type::Int,
            "Float" => Type::Float,
            "String" => Type::String,
            "Boolean" => Type::Boolean,
            "Void" => Type::Void,
            _ => {
                if self.classes.contains_key(name) {
                    Type::Instance(name.to_string())
                } else {
                    self.error(span, &format!("Unknown type '{}'.", name));
                    Type::Error
                }
            }
        }
    }

    fn error(&mut self, span: Span, message: &str) {
        self.errors.push(TypeError {
            message: message.to_string(),
            span,
        });
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
