use ast::{Expr, ExprKind, Stmt, StmtKind, Span, BinaryOp, UnaryOp};
use crate::types::Type;
use crate::env::TypeEnvironment;

#[derive(Debug)]
pub struct TypeError {
    pub message: String,
    pub span: Span,
}

pub struct TypeChecker {
    env: TypeEnvironment,
    pub errors: Vec<TypeError>,
    current_return_type: Option<Type>,
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            env: TypeEnvironment::new(),
            errors: Vec::new(),
            current_return_type: None,
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
            StmtKind::Let { name, initializer } | StmtKind::Var { name, initializer } => {
                let init_type = self.check_expr(initializer);
                self.env.declare(name.clone(), init_type);
            }
            StmtKind::Func { name, params, return_type, body } => {
                // Parse return type from AST string
                let ret_ty = if let Some(rt) = return_type {
                    self.parse_type(rt, stmt.span)
                } else {
                    Type::Void
                };

                // Functions are declared in the current scope
                // (Assuming functions are just a special type of variable for now, though full Pace has distinct func tracking)
                // For this basic pass, we won't strictly enforce function type tracking yet, just their body types.
                self.env.declare(name.clone(), Type::Void); // Placeholder for function type

                self.env.push_scope();
                for (param_name, param_type_str) in params {
                    let param_ty = self.parse_type(param_type_str, stmt.span);
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
                    Type::Error // Resolver should have caught this, but fallback to Error
                }
            }
            ExprKind::Grouping(inner) => {
                self.check_expr(inner)
            }
            ExprKind::Call { callee, arguments } => {
                let callee_type = self.check_expr(callee);
                for arg in arguments {
                    self.check_expr(arg);
                }
                
                match callee_type {
                    Type::BuiltinFunc => Type::Void,
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
                self.error(span, &format!("Unknown type '{}'.", name));
                Type::Error
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
            initializer: Expr::new(ExprKind::Binary(
                Box::new(Expr::new(ExprKind::Integer(10), make_span())),
                BinaryOp::Add,
                Box::new(Expr::new(ExprKind::Integer(5), make_span())),
            ), make_span()),
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
            initializer: Expr::new(ExprKind::Binary(
                Box::new(Expr::new(ExprKind::Integer(10), make_span())),
                BinaryOp::Add,
                Box::new(Expr::new(ExprKind::String("hello".into()), make_span())),
            ), make_span()),
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
