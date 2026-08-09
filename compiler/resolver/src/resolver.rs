use ast::{Expr, ExprKind, Stmt, StmtKind, Span};
use crate::scope::ScopeStack;

#[derive(Debug)]
pub struct ResolverError {
    pub message: String,
    pub span: Span,
}

pub struct Resolver {
    scopes: ScopeStack,
    pub errors: Vec<ResolverError>,
}

impl Resolver {
    pub fn new() -> Self {
        let mut scopes = ScopeStack::new();
        scopes.declare("print".into());
        Self {
            scopes,
            errors: Vec::new(),
        }
    }

    pub fn resolve(&mut self, statements: &[Stmt]) {
        for stmt in statements {
            self.resolve_stmt(stmt);
        }
    }

    fn resolve_stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Block(stmts) => {
                self.scopes.push_scope();
                self.resolve(stmts);
                self.scopes.pop_scope();
            }
            StmtKind::Let { name, type_annotation: _, initializer } | StmtKind::Var { name, type_annotation: _, initializer } => {
                // Resolve initializer first so it can't reference the variable being declared
                if let Some(init) = initializer {
                    self.resolve_expr(init);
                }
                
                if !self.scopes.declare(name.clone()) {
                    self.error(stmt.span, &format!("Variable '{}' is already declared in this scope.", name));
                }
            }
            StmtKind::Class { name, methods, fields } => {
                if !self.scopes.declare(name.clone()) {
                    self.error(stmt.span, &format!("Class '{}' is already declared in this scope.", name));
                }

                self.scopes.push_scope();
                self.scopes.declare("self".into());
                
                for field in fields {
                    self.resolve_stmt(field);
                }
                for method in methods {
                    self.resolve_stmt(method);
                }
                
                self.scopes.pop_scope();
            }
            StmtKind::Func { name, params, body, .. } => {
                if !self.scopes.declare(name.clone()) {
                    self.error(stmt.span, &format!("Function '{}' is already declared in this scope.", name));
                }

                self.scopes.push_scope();
                for (param_name, _) in params {
                    if !self.scopes.declare(param_name.clone()) {
                        self.error(stmt.span, &format!("Parameter '{}' is declared multiple times.", param_name));
                    }
                }
                
                self.resolve_stmt(body);
                self.scopes.pop_scope();
            }
            StmtKind::If { condition, then_branch, else_branch } => {
                self.resolve_expr(condition);
                self.resolve_stmt(then_branch);
                if let Some(e_branch) = else_branch {
                    self.resolve_stmt(e_branch);
                }
            }
            StmtKind::While { condition, body } => {
                self.resolve_expr(condition);
                self.resolve_stmt(body);
            }
            StmtKind::For { item_name, iterator, body } => {
                self.resolve_expr(iterator);

                self.scopes.push_scope();
                self.scopes.declare(item_name.clone());
                self.resolve_stmt(body);
                self.scopes.pop_scope();
            }
            StmtKind::Expression(expr) => {
                self.resolve_expr(expr);
            }
            StmtKind::Return { value } => {
                if let Some(val) = value {
                    self.resolve_expr(val);
                }
            }
        }
    }

    fn resolve_expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Variable(name) => {
                if !self.scopes.resolve(name) {
                    self.error(expr.span, &format!("Cannot find variable '{}' in this scope.", name));
                }
            }
            ExprKind::Binary(left, _, right) => {
                self.resolve_expr(left);
                self.resolve_expr(right);
            }
            ExprKind::Unary(_, right) => {
                self.resolve_expr(right);
            }
            ExprKind::Grouping(inner) => {
                self.resolve_expr(inner);
            }
            ExprKind::Call { callee, arguments } => {
                self.resolve_expr(callee);
                for arg in arguments {
                    self.resolve_expr(arg);
                }
            }
            ExprKind::Get { object, name: _ } => {
                self.resolve_expr(object);
            }
            ExprKind::Set { object, name: _, value } => {
                self.resolve_expr(object);
                self.resolve_expr(value);
            }
            ExprKind::Assign { name, value } => {
                if !self.scopes.resolve(name) {
                    self.error(expr.span, &format!("Cannot assign to undefined variable '{}'.", name));
                }
                self.resolve_expr(value);
            }
            ExprKind::SelfRef => {
                if !self.scopes.resolve(&"self".to_string()) {
                    self.error(expr.span, "Cannot use 'self' outside of a class method.");
                }
            }
            // Literals have no names to resolve
            ExprKind::Integer(_) | ExprKind::Float(_) | ExprKind::String(_) | ExprKind::Boolean(_) => {}
        }
    }

    fn error(&mut self, span: Span, message: &str) {
        self.errors.push(ResolverError {
            message: message.to_string(),
            span,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ast::{ExprKind, StmtKind, Location};

    fn make_span() -> Span {
        Span::new(0, 0, Location::new(1, 1), Location::new(1, 1))
    }

    #[test]
    fn test_valid_shadowing() {
        // let x = 1; { let x = 2; print(x); }
        let outer_let = Stmt::new(StmtKind::Let {
            name: "x".into(),
            type_annotation: None,
            initializer: Some(Expr::new(ExprKind::Integer(1), make_span())),
        }, make_span());

        let inner_let = Stmt::new(StmtKind::Let {
            name: "x".into(),
            type_annotation: None,
            initializer: Some(Expr::new(ExprKind::Integer(2), make_span())),
        }, make_span());

        let inner_usage = Stmt::new(StmtKind::Expression(Expr::new(ExprKind::Variable("x".into()), make_span())), make_span());

        let block = Stmt::new(StmtKind::Block(vec![inner_let, inner_usage]), make_span());

        let mut resolver = Resolver::new();
        resolver.resolve(&[outer_let, block]);
        
        assert!(resolver.errors.is_empty(), "Expected no errors, got: {:?}", resolver.errors);
    }

    #[test]
    fn test_invalid_redeclaration() {
        // let x = 1; let x = 2;
        let let1 = Stmt::new(StmtKind::Let {
            name: "x".into(),
            type_annotation: None,
            initializer: Some(Expr::new(ExprKind::Integer(1), make_span())),
        }, make_span());

        let let2 = Stmt::new(StmtKind::Let {
            name: "x".into(),
            type_annotation: None,
            initializer: Some(Expr::new(ExprKind::Integer(2), make_span())),
        }, make_span());

        let mut resolver = Resolver::new();
        resolver.resolve(&[let1, let2]);
        
        assert_eq!(resolver.errors.len(), 1);
        assert!(resolver.errors[0].message.contains("already declared"));
    }

    #[test]
    fn test_undefined_variable() {
        // print(y);
        let usage = Stmt::new(StmtKind::Expression(Expr::new(ExprKind::Variable("y".into()), make_span())), make_span());

        let mut resolver = Resolver::new();
        resolver.resolve(&[usage]);
        
        assert_eq!(resolver.errors.len(), 1);
        assert!(resolver.errors[0].message.contains("Cannot find variable 'y'"));
    }
}
