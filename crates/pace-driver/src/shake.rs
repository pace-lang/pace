use pace_ast::{Expr, Stmt, TypeAnnotation};
use std::collections::HashSet;

pub struct TreeShaker {
    reachable: HashSet<ustr::Ustr>,
}

impl Default for TreeShaker {
    fn default() -> Self {
        Self::new()
    }
}

impl TreeShaker {
    pub fn new() -> Self {
        Self {
            reachable: HashSet::new(),
        }
    }

    pub fn run(
        arena: &mut pace_ast::arena::AstArena,
        ast: Vec<pace_ast::arena::StmtId>,
    ) -> Vec<pace_ast::arena::StmtId> {
        let mut shaker = Self::new();
        // Start from main
        shaker.reachable.insert(ustr::Ustr::from("main"));

        // Build an index of all declarations
        let mut decls = std::collections::HashMap::new();
        shaker.index_decls(arena, &ast, &mut decls);

        // Iteratively trace reachable symbols until fixed point
        let mut queue: Vec<ustr::Ustr> = vec![ustr::Ustr::from("main")];

        while let Some(current) = queue.pop() {
            if let Some(&stmt_id) = decls.get(&current) {
                shaker.trace_stmt(arena, stmt_id, &mut queue);
            }
        }

        // Filter out unreachable declarations
        shaker.filter_ast(arena, ast)
    }

    fn index_decls(
        &self,
        arena: &pace_ast::arena::AstArena,
        ast: &[pace_ast::arena::StmtId],
        decls: &mut std::collections::HashMap<ustr::Ustr, pace_ast::arena::StmtId>,
    ) {
        for stmt_id in ast {
            let stmt = arena.get_stmt(*stmt_id);
            match stmt {
                Stmt::FuncDecl { name, .. }
                | Stmt::ClassDecl { name, .. }
                | Stmt::ActorDecl { name, .. }
                | Stmt::StructDecl { name, .. }
                | Stmt::EnumDecl { name, .. }
                | Stmt::InterfaceDecl { name, .. } => {
                    decls.insert(*name, *stmt_id);
                }
                Stmt::Module { body, .. } => {
                    self.index_decls(arena, body, decls);
                }
                _ => {}
            }
        }
    }

    fn filter_ast(
        &self,
        arena: &mut pace_ast::arena::AstArena,
        ast: Vec<pace_ast::arena::StmtId>,
    ) -> Vec<pace_ast::arena::StmtId> {
        let mut new_ast = Vec::new();
        for stmt_id in ast {
            let stmt = arena.get_stmt(stmt_id).clone();
            match stmt {
                Stmt::Module { name, body } => {
                    let filtered_body = self.filter_ast(arena, body);
                    let new_mod_id = arena.alloc_stmt(Stmt::Module {
                        name,
                        body: filtered_body,
                    }, pace_ast::Span::default());
                    new_ast.push(new_mod_id);
                }
                Stmt::FuncDecl { name, .. }
                | Stmt::ClassDecl { name, .. }
                | Stmt::ActorDecl { name, .. }
                | Stmt::StructDecl { name, .. }
                | Stmt::EnumDecl { name, .. }
                | Stmt::InterfaceDecl { name, .. } => {
                    if self.reachable.contains(&name) {
                        new_ast.push(stmt_id);
                    }
                }
                _ => new_ast.push(stmt_id),
            }
        }
        new_ast
    }

    fn trace_stmt(
        &mut self,
        arena: &pace_ast::arena::AstArena,
        stmt_id: pace_ast::arena::StmtId,
        queue: &mut Vec<ustr::Ustr>,
    ) {
        let stmt = arena.get_stmt(stmt_id);
        match stmt {
            Stmt::Module { body, .. } => {
                for s in body {
                    self.trace_stmt(arena, *s, queue);
                }
            }
            Stmt::Expr(expr) => self.trace_expr(arena, *expr, queue),
            Stmt::VarDecl {
                initializer,
                type_annotation,
                ..
            } => {
                if let Some(expr) = initializer {
                    self.trace_expr(arena, *expr, queue);
                }
                if let Some(ty) = type_annotation {
                    self.trace_type(ty, queue);
                }
            }
            Stmt::Return(expr) => {
                if let Some(e) = expr {
                    self.trace_expr(arena, *e, queue);
                }
            }
            Stmt::FuncDecl {
                body,
                params,
                return_type,
                ..
            } => {
                for p in params {
                    self.trace_type(&p.type_annotation, queue);
                }
                if let Some(ty) = return_type {
                    self.trace_type(ty, queue);
                }
                for s in body {
                    self.trace_stmt(arena, *s, queue);
                }
            }
            Stmt::ClassDecl {
                fields,
                methods,
                implements,
                ..
            }
            | Stmt::ActorDecl {
                fields,
                methods,
                implements,
                ..
            } => {
                for f in fields {
                    self.trace_stmt(arena, *f, queue);
                }
                for m in methods {
                    self.trace_stmt(arena, *m, queue);
                }
                if let Some(imp) = implements {
                    self.trace_type(imp, queue);
                }
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.trace_expr(arena, *condition, queue);
                self.trace_stmt(arena, *then_branch, queue);
                if let Some(eb) = else_branch {
                    self.trace_stmt(arena, *eb, queue);
                }
            }
            Stmt::While { condition, body } => {
                self.trace_expr(arena, *condition, queue);
                self.trace_stmt(arena, *body, queue);
            }
            Stmt::Block(stmts) => {
                for s in stmts {
                    self.trace_stmt(arena, *s, queue);
                }
            }
            Stmt::Match { expr, arms } => {
                self.trace_expr(arena, *expr, queue);
                for (_, body) in arms {
                    self.trace_stmt(arena, *body, queue);
                }
            }
            _ => {}
        }
    }

    fn trace_expr(
        &mut self,
        arena: &pace_ast::arena::AstArena,
        expr_id: pace_ast::arena::ExprId,
        queue: &mut Vec<ustr::Ustr>,
    ) {
        let expr = arena.get_expr(expr_id);
        match expr {
            Expr::Call { callee, args } => {
                if let Expr::Identifier(name, _) = arena.get_expr(*callee)
                    && !self.reachable.contains(name)
                {
                    self.reachable.insert(*name);
                    queue.push(*name);
                }
                self.trace_expr(arena, *callee, queue);
                for arg in args {
                    self.trace_expr(arena, *arg, queue);
                }
            }
            Expr::Identifier(name, _) => {
                if !self.reachable.contains(name) {
                    self.reachable.insert(*name);
                    queue.push(*name);
                }
            }
            Expr::Binary { left, right, .. } => {
                self.trace_expr(arena, *left, queue);
                self.trace_expr(arena, *right, queue);
            }
            Expr::Assign { target, value } => {
                self.trace_expr(arena, *target, queue);
                self.trace_expr(arena, *value, queue);
            }
            Expr::MemberAccess {
                object,
                property: _,
                computed_class: _,
                is_static_operator: _,
            } => {
                self.trace_expr(arena, *object, queue);
            }
            Expr::OptionalMemberAccess { object, .. } => {
                self.trace_expr(arena, *object, queue);
            }
            Expr::GenericInstantiation {
                callee,
                generic_args,
            } => {
                self.trace_expr(arena, *callee, queue);
                for arg in generic_args {
                    self.trace_type(arg, queue);
                }
            }
            Expr::Try(expr) | Expr::Unwrap(expr) | Expr::Await(expr) => {
                self.trace_expr(arena, *expr, queue)
            }
            Expr::InterpolatedString(args) => {
                for arg in args {
                    self.trace_expr(arena, *arg, queue);
                }
            }
            Expr::NullCoalesce { left, right } => {
                self.trace_expr(arena, *left, queue);
                self.trace_expr(arena, *right, queue);
            }
            _ => {}
        }
    }

    fn trace_type(&mut self, ty: &TypeAnnotation, queue: &mut Vec<ustr::Ustr>) {
        if !self.reachable.contains(&ty.name) {
            self.reachable.insert(ty.name);
            queue.push(ty.name);
        }
        for arg in &ty.args {
            self.trace_type(arg, queue);
        }
    }
}
