use pace_ast::{Stmt, Expr, TypeAnnotation};
use std::collections::HashSet;

pub struct TreeShaker {
    reachable: HashSet<String>,
}

impl TreeShaker {
    pub fn new() -> Self {
        Self {
            reachable: HashSet::new(),
        }
    }

    pub fn run(ast: Vec<Stmt>) -> Vec<Stmt> {
        let mut shaker = Self::new();
        // Start from main
        shaker.reachable.insert("main".to_string());
        
        // Build an index of all declarations
        let mut decls = std::collections::HashMap::new();
        for stmt in &ast {
            if let Stmt::FuncDecl { name, .. } |
                   Stmt::ClassDecl { name, .. } | Stmt::ActorDecl { name, .. } |
                   Stmt::StructDecl { name, .. } |
                   Stmt::EnumDecl { name, .. } |
                   Stmt::InterfaceDecl { name, .. } = stmt {
                decls.insert(name.clone(), stmt.clone());
            }
        }
        
        // Iteratively trace reachable symbols until fixed point
        let mut queue = vec!["main".to_string()];
        
        while let Some(current) = queue.pop() {
            if let Some(decl) = decls.get(&current) {
                shaker.trace_stmt(decl, &mut queue);
            }
        }
        
        // Filter out unreachable declarations
        ast.into_iter().filter(|stmt| {
            match stmt {
                Stmt::FuncDecl { name, .. } |
                Stmt::ClassDecl { name, .. } | Stmt::ActorDecl { name, .. } |
                Stmt::StructDecl { name, .. } |
                Stmt::EnumDecl { name, .. } |
                Stmt::InterfaceDecl { name, .. } => shaker.reachable.contains(name),
                _ => true, // Keep expressions, variable declarations in top level, etc.
            }
        }).collect()
    }

    fn trace_stmt(&mut self, stmt: &Stmt, queue: &mut Vec<String>) {
        match stmt {
            Stmt::Expr(expr) => self.trace_expr(expr, queue),
            Stmt::VarDecl { initializer, type_annotation, .. } => {
                if let Some(expr) = initializer {
                    self.trace_expr(expr, queue);
                }
                if let Some(ty) = type_annotation {
                    self.trace_type(ty, queue);
                }
            }
            Stmt::Return(expr) => {
                if let Some(e) = expr {
                    self.trace_expr(e, queue);
                }
            }
            Stmt::FuncDecl { body, params, return_type, .. } => {
                for p in params {
                    self.trace_type(&p.type_annotation, queue);
                }
                if let Some(ty) = return_type {
                    self.trace_type(ty, queue);
                }
                for s in body {
                    self.trace_stmt(s, queue);
                }
            }
            Stmt::ClassDecl { fields, methods, implements, .. } | Stmt::ActorDecl { fields, methods, implements, .. } => {
                for f in fields { self.trace_stmt(f, queue); }
                for m in methods { self.trace_stmt(m, queue); }
                if let Some(imp) = implements { self.trace_type(imp, queue); }
            }
            Stmt::If { condition, then_branch, else_branch } => {
                self.trace_expr(condition, queue);
                self.trace_stmt(then_branch, queue);
                if let Some(eb) = else_branch { self.trace_stmt(eb, queue); }
            }
            Stmt::While { condition, body } => {
                self.trace_expr(condition, queue);
                self.trace_stmt(body, queue);
            }
            Stmt::Block(stmts) => {
                for s in stmts { self.trace_stmt(s, queue); }
            }
            Stmt::Match { expr, arms } => {
                self.trace_expr(expr, queue);
                for (_, body) in arms {
                    self.trace_stmt(body, queue);
                }
            }
            _ => {}
        }
    }

    fn trace_expr(&mut self, expr: &Expr, queue: &mut Vec<String>) {
        match expr {
            Expr::Call { callee, args } => {
                if let Expr::Identifier(name) = &**callee {
                    if !self.reachable.contains(name) {
                        self.reachable.insert(name.clone());
                        queue.push(name.clone());
                    }
                }
                self.trace_expr(callee, queue);
                for arg in args {
                    self.trace_expr(arg, queue);
                }
            }
            Expr::Identifier(name) => {
                if !self.reachable.contains(name) {
                    self.reachable.insert(name.clone());
                    queue.push(name.clone());
                }
            }
            Expr::Binary { left, right, .. } => {
                self.trace_expr(left, queue);
                self.trace_expr(right, queue);
            }
            Expr::Assign { target, value } => {
                self.trace_expr(target, queue);
                self.trace_expr(value, queue);
            }
            Expr::MemberAccess { object, property: _, computed_class: _, is_static_operator: _ } => {
                self.trace_expr(object, queue);
            }
            Expr::OptionalMemberAccess { object, .. } => {
                self.trace_expr(object, queue);
            }
            Expr::GenericInstantiation { callee, generic_args } => {
                self.trace_expr(callee, queue);
                for arg in generic_args {
                    self.trace_type(arg, queue);
                }
            }
            Expr::Try(expr) | Expr::Unwrap(expr) | Expr::Await(expr) => self.trace_expr(expr, queue),
            Expr::InterpolatedString(args) => {
                for arg in args {
                    self.trace_expr(arg, queue);
                }
            }
            Expr::NullCoalesce { left, right } => {
                self.trace_expr(left, queue);
                self.trace_expr(right, queue);
            }
            _ => {}
        }
    }

    fn trace_type(&mut self, ty: &TypeAnnotation, queue: &mut Vec<String>) {
        if !self.reachable.contains(&ty.name) {
            self.reachable.insert(ty.name.clone());
            queue.push(ty.name.clone());
        }
        for arg in &ty.args {
            self.trace_type(arg, queue);
        }
    }
}
