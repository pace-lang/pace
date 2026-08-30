use pace_ast::{Expr, Stmt};
use std::collections::HashSet;

pub struct EscapeAnalysis {
    /// Variables that are captured by closures
    pub escaped_vars: HashSet<ustr::Ustr>,

    /// Stack of scopes. Each scope contains (var_name, closure_depth)
    scope_stack: Vec<Vec<(String, usize)>>,

    current_closure_depth: usize,
}

impl EscapeAnalysis {
    pub fn new() -> Self {
        Self {
            escaped_vars: HashSet::new(),
            scope_stack: vec![Vec::new()],
            current_closure_depth: 0,
        }
    }

    pub fn analyze_function(stmts: &[Stmt]) -> HashSet<ustr::Ustr> {
        let mut analyzer = Self::new();
        for stmt in stmts {
            analyzer.visit_stmt(stmt);
        }
        analyzer.escaped_vars
    }

    fn push_scope(&mut self) {
        self.scope_stack.push(Vec::new());
    }

    fn pop_scope(&mut self) {
        self.scope_stack.pop();
    }

    fn declare_var(&mut self, name: &str) {
        if let Some(scope) = self.scope_stack.last_mut() {
            scope.push((name.to_string(), self.current_closure_depth));
        }
    }

    fn reference_var(&mut self, name: &str) {
        // Find where the variable was declared
        for scope in self.scope_stack.iter().rev() {
            for (var_name, depth) in scope.iter().rev() {
                if var_name == name {
                    // If it was declared at a lower closure depth, it is captured!
                    if *depth < self.current_closure_depth {
                        self.escaped_vars.insert(ustr::Ustr::from(name));
                    }
                    return;
                }
            }
        }
    }

    fn visit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::VarDecl {
                name, initializer, ..
            } => {
                if let Some(init) = initializer {
                    self.visit_expr(init);
                }
                self.declare_var(name);
            }
            Stmt::Block(stmts) => {
                self.push_scope();
                for s in stmts {
                    self.visit_stmt(s);
                }
                self.pop_scope();
            }
            Stmt::Expr(expr) | Stmt::Return(Some(expr)) => {
                self.visit_expr(expr);
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.visit_expr(condition);
                self.visit_stmt(then_branch);
                if let Some(e) = else_branch {
                    self.visit_stmt(e);
                }
            }
            Stmt::While { condition, body } => {
                self.visit_expr(condition);
                self.visit_stmt(body);
            }
            Stmt::Loop { body } => {
                self.visit_stmt(body);
            }
            _ => {}
        }
    }

    fn visit_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Identifier(name, _) => {
                self.reference_var(name);
            }
            Expr::Assign { target, value } => {
                self.visit_expr(target);
                self.visit_expr(value);
            }
            Expr::Binary { left, right, .. } => {
                self.visit_expr(left);
                self.visit_expr(right);
            }
            Expr::Try(right) | Expr::Await(right) | Expr::Unwrap(right) => {
                self.visit_expr(right);
            }
            Expr::Call { callee, args } => {
                self.visit_expr(callee);
                for arg in args {
                    self.visit_expr(arg);
                }
            }
            Expr::MemberAccess { object, .. } => {
                self.visit_expr(object);
            }
            Expr::OptionalMemberAccess { object, .. } => {
                self.visit_expr(object);
            }
            Expr::NullCoalesce { left, right } => {
                self.visit_expr(left);
                self.visit_expr(right);
            }
            Expr::GenericInstantiation { callee, .. } => {
                self.visit_expr(callee);
            }
            Expr::InterpolatedString(parts) => {
                for part in parts {
                    self.visit_expr(part);
                }
            }
            Expr::Block(stmts) => {
                self.push_scope();
                for s in stmts {
                    self.visit_stmt(s);
                }
                self.pop_scope();
            }
            Expr::Closure { params, body, .. } => {
                self.current_closure_depth += 1;
                self.push_scope();

                for (param_name, _) in params {
                    self.declare_var(param_name);
                }

                self.visit_expr(body);

                self.pop_scope();
                self.current_closure_depth -= 1;
            }
            _ => {}
        }
    }
}
