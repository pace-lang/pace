use module::graph::ModuleGraph;
use module::module::Module;
use module::module_id::ModuleId;
use std::collections::HashMap;

use crate::scope::ScopeStack;
use ast::{Expr, ExprKind, Span, Stmt, StmtKind};
use diagnostics::{Diagnostic, DiagnosticBuilder, DiagnosticCode};

pub struct Resolver<'a> {
    pub session: &'a session::CompilerSession,
    scopes: ScopeStack,
    pub errors: Vec<Diagnostic>,
    module_exports: HashMap<ModuleId, HashMap<session::Symbol, Vec<session::Symbol>>>,
}

impl<'a> Resolver<'a> {
    pub fn new(session: &'a session::CompilerSession) -> Self {
        let mut scopes = ScopeStack::new();
        scopes.declare(session.interner.borrow_mut().intern("print"));
        scopes.declare(session.interner.borrow_mut().intern("Result"));
        scopes.declare(session.interner.borrow_mut().intern("Ok"));
        scopes.declare(session.interner.borrow_mut().intern("Err"));
        scopes.declare(session.interner.borrow_mut().intern("Option"));
        scopes.declare(session.interner.borrow_mut().intern("Some"));
        scopes.declare(session.interner.borrow_mut().intern("None"));
        Self {
            session,
            scopes,
            errors: Vec::new(),
            module_exports: HashMap::new(),
        }
    }

    pub fn resolve_graph(&mut self, graph: &ModuleGraph) {
        // Collect exports for all modules first
        for module in graph.modules() {
            let mut exports = HashMap::new();
            for stmt in &module.ast {
                match &stmt.kind {
                    StmtKind::Binding {
                        name,
                        is_private: false,
                        ..
                    }
                    | StmtKind::Class {
                        name,
                        is_private: false,
                        ..
                    }
                    | StmtKind::Actor {
                        name,
                        is_private: false,
                        ..
                    }
                    | StmtKind::Struct {
                        name,
                        is_private: false,
                        ..
                    }
                    | StmtKind::Interface {
                        name,
                        is_private: false,
                        ..
                    }
                    | StmtKind::ForeignFunc {
                        name,
                        is_private: false,
                        ..
                    }
                    | StmtKind::TypeAlias {
                        name,
                        is_private: false,
                        ..
                    }
                    | StmtKind::Func {
                        name,
                        is_private: false,
                        ..
                    } => {
                        exports.insert(*name, Vec::new());
                    }
                    StmtKind::Enum {
                        name,
                        is_private: false,
                        variants,
                        ..
                    } => {
                        let mut sub_exports = Vec::new();
                        for variant in variants {
                            sub_exports.push(variant.name);
                        }
                        exports.insert(*name, sub_exports);
                    }
                    // We don't fully implement re-exports (StmtKind::Export) just yet
                    _ => {}
                }
            }
            self.module_exports.insert(module.id, exports);
        }

        // Now resolve each module
        for module in graph.modules() {
            self.resolve_module(module, graph);
        }
    }

    fn resolve_module(&mut self, module: &Module, graph: &ModuleGraph) {
        self.scopes.push_scope(); // Module scope

        // Resolve imports and inject into module scope
        for stmt in &module.ast {
            if let StmtKind::Import {
                path,
                alias,
            } = &stmt.kind
            {
                // Find the imported module
                let clean_path = self
                    .session
                    .interner
                    .borrow()
                    .lookup(*path)
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string();
                let imported_id = graph.resolve_import(module.id, &clean_path);

                if let Some(id) = imported_id {
                    if let Some(exports) = self.module_exports.get(&id) {
                        if let Some(alias_name) = alias {
                            // alias imports act as a namespace, but for now we just declare the alias
                            // In a real implementation, we'd bind it to a module object.
                            self.scopes.declare(*alias_name);
                        } else {
                            for (export, sub_exports) in exports {

                                self.scopes.declare(*export);
                                for sub in sub_exports {
                                    self.scopes.declare(*sub);
                                }
                            }
                        }
                    }
                } else {
                    self.error(
                        stmt.span,
                        DiagnosticCode::UnknownIdentifier,
                        &format!(
                            "Cannot resolve import '{}'",
                            self.session.interner.borrow().lookup(*path)
                        ),
                    );
                }
            }
        }

        self.hoist_declarations(&module.ast);
        self.resolve(&module.ast);
        self.scopes.pop_scope();
    }

    pub fn hoist_declarations(&mut self, statements: &[Stmt]) {
        for stmt in statements {
            match &stmt.kind {
                StmtKind::Func { name, .. }
                | StmtKind::ForeignFunc { name, .. }
                | StmtKind::Class { name, .. }
                | StmtKind::Actor { name, .. }
                | StmtKind::Struct { name, .. }
                | StmtKind::TypeAlias { name, .. }
                | StmtKind::Interface { name, .. } => {
                    if !self.scopes.declare(*name) {
                        self.error(
                            stmt.span,
                            DiagnosticCode::DuplicateDeclaration,
                            &format!(
                                "'{}' is already declared in this scope.",
                                self.session.interner.borrow().lookup(*name)
                            ),
                        );
                    }
                }
                StmtKind::Enum { name, variants, .. } => {
                    if !self.scopes.declare(*name) {
                        self.error(
                            stmt.span,
                            DiagnosticCode::DuplicateDeclaration,
                            &format!(
                                "Enum '{}' is already declared in this scope.",
                                self.session.interner.borrow().lookup(*name)
                            ),
                        );
                    }
                    for variant in variants {
                        if !self.scopes.declare(variant.name) {
                            self.error(
                                stmt.span,
                                DiagnosticCode::DuplicateDeclaration,
                                &format!(
                                    "Enum variant '{}' is already declared.",
                                    self.session.interner.borrow().lookup(variant.name)
                                ),
                            );
                        }
                    }
                }
                _ => {}
            }
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
                self.hoist_declarations(stmts);
                self.resolve(stmts);
                self.scopes.pop_scope();
            }
            StmtKind::Binding {
                name,
                type_annotation: _,
                initializer,
                mutability: _,
                is_weak: _,
                is_private: _,
                is_static: _,
            } => {
                // Resolve initializer first so it can't reference the variable being declared
                if let Some(init) = initializer {
                    self.resolve_expr(init);
                }

                if !self.scopes.declare(*name) {
                    self.error(
                        stmt.span,
                        DiagnosticCode::DuplicateDeclaration,
                        &format!(
                            "Variable '{}' is already declared in this scope.",
                            self.session.interner.borrow().lookup(*name)
                        ),
                    );
                }
            }
            StmtKind::Class {
                name: _,
                type_params: _,
                implements: _,
                methods,
                fields,
                is_private: _,
            }
            | StmtKind::Actor {
                name: _,
                type_params: _,
                implements: _,
                methods,
                fields,
                is_private: _,
            }
            | StmtKind::Struct {
                name: _,
                type_params: _,
                methods,
                fields,
                is_private: _,
            } => {
                // Name declared in hoisting
                self.scopes.push_scope();
                self.scopes
                    .declare(self.session.interner.borrow_mut().intern("self"));

                for field in fields {
                    self.resolve_stmt(field);
                }
                for method in methods {
                    self.resolve_stmt(method);
                }

                self.scopes.pop_scope();
            }
            StmtKind::Extension {
                target_type: _,
                type_params: _,
                methods,
            } => {
                self.scopes.push_scope();
                self.scopes
                    .declare(self.session.interner.borrow_mut().intern("self"));

                for method in methods {
                    self.resolve_stmt(method);
                }

                self.scopes.pop_scope();
            }
            StmtKind::Interface {
                name: _,
                type_params,
                methods: _,
                is_private: _,
            } => {
                if !type_params.is_empty() {}
                // Name declared in hoisting
            }
            StmtKind::Enum {
                name: _,
                type_params: _,
                variants: _,
                methods: _,
                is_private: _,
            } => {
                // Handled in hoisting pass
            }
            StmtKind::ForeignFunc {
                name: _,
                base_name: _,
                type_params: _,
                params: _,
                return_type: _,
                is_private: _,
                is_static: _,
            } => {
                // Name declared in hoisting
            }
            StmtKind::TypeAlias {
                name: _,
                type_params: _,
                target_type: _,
                is_private: _,
            } => {
                // Name declared in hoisting
            }
            StmtKind::Func {
                name: _,
                type_params: _,
                params,
                return_type: _,
                body,
                is_private: _,
                is_async: _,
                is_static: _,
            } => {
                // Name declared in hoisting

                self.scopes.push_scope();
                for (param_name, _) in params {
                    if !self.scopes.declare(*param_name) {
                        self.error(
                            stmt.span,
                            DiagnosticCode::DuplicateDeclaration,
                            &format!(
                                "Parameter '{}' is declared multiple times.",
                                self.session.interner.borrow().lookup(*param_name)
                            ),
                        );
                    }
                }

                self.resolve_stmt(body);
                self.scopes.pop_scope();
            }
            StmtKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
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
            StmtKind::For {
                item_name,
                iterator,
                body,
            } => {
                self.resolve_expr(iterator);

                self.scopes.push_scope();
                self.scopes.declare(*item_name);
                self.resolve_stmt(body);
                self.scopes.pop_scope();
            }
            StmtKind::Import { .. } | StmtKind::Export { .. } => {}
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
            ExprKind::PostfixTry(inner) => self.resolve_expr(inner),
            ExprKind::Variable(name) => {
                if !self.scopes.resolve(*name) {
                    self.error(
                        expr.span,
                        DiagnosticCode::UnknownIdentifier,
                        &format!(
                            "Cannot find variable '{}' in this scope.",
                            self.session.interner.borrow().lookup(*name)
                        ),
                    );
                }
            }
            ExprKind::Range { start, end } => {
                self.resolve_expr(start);
                self.resolve_expr(end);
            }
            ExprKind::Binary(left, _, right) | ExprKind::Logical(left, _, right) => {
                self.resolve_expr(left);
                self.resolve_expr(right);
            }
            ExprKind::CompoundAssign { target, value, .. } => {
                self.resolve_expr(target);
                self.resolve_expr(value);
            }
            ExprKind::Unary(_, right) => {
                self.resolve_expr(right);
            }
            ExprKind::Grouping(inner) => {
                self.resolve_expr(inner);
            }
            ExprKind::Call {
                callee,
                type_args: _,
                arguments,
            } => {
                self.resolve_expr(callee);
                for arg in arguments {
                    self.resolve_expr(arg);
                }
            }
            ExprKind::Get { object, name: _ } => {
                self.resolve_expr(object);
            }
            ExprKind::Set {
                object,
                name: _,
                value,
            } => {
                self.resolve_expr(object);
                self.resolve_expr(value);
            }
            ExprKind::Assign { name, value } => {
                if !self.scopes.resolve(*name) {
                    self.error(
                        expr.span,
                        DiagnosticCode::UnknownIdentifier,
                        &format!(
                            "Cannot assign to undefined variable '{}'.",
                            self.session.interner.borrow().lookup(*name)
                        ),
                    );
                }
                self.resolve_expr(value);
            }
            ExprKind::Array(elements) => {
                for el in elements {
                    self.resolve_expr(el);
                }
            }
            ExprKind::ArrayRepeat { value, count } => {
                self.resolve_expr(value);
                self.resolve_expr(count);
            }

            ExprKind::InterpolatedString(pieces) => {
                for piece in pieces {
                    self.resolve_expr(piece);
                }
            }
            ExprKind::IndexGet { object, index } => {
                self.resolve_expr(object);
                self.resolve_expr(index);
            }
            ExprKind::IndexSet {
                object,
                index,
                value,
            } => {
                self.resolve_expr(object);
                self.resolve_expr(index);
                self.resolve_expr(value);
            }
            ExprKind::Await(inner) | ExprKind::Spawn(inner) => {
                self.resolve_expr(inner);
            }
            ExprKind::SelfRef => {
                if !self
                    .scopes
                    .resolve(self.session.interner.borrow_mut().intern("self"))
                {
                    self.error(
                        expr.span,
                        DiagnosticCode::UnknownIdentifier,
                        "Cannot use 'self' outside of a class method.",
                    );
                }
            }
            // Literals have no names to resolve
            ExprKind::Integer(_)
            | ExprKind::Float(_)
            | ExprKind::String(_)
            | ExprKind::Bool(_)
            | ExprKind::Null => {}
            ExprKind::ForceUnwrap(inner) => {
                self.resolve_expr(inner);
            }
            ExprKind::OptionalGet { object, name: _ } => {
                self.resolve_expr(object);
            }
            ExprKind::NullCoalesce { left, right } => {
                self.resolve_expr(left);
                self.resolve_expr(right);
            }
            ExprKind::NullCoalesceAssign { left, right } => {
                self.resolve_expr(left);
                self.resolve_expr(right);
            }
            ExprKind::Ternary {
                condition,
                true_expr,
                false_expr,
            } => {
                self.resolve_expr(condition);
                self.resolve_expr(true_expr);
                self.resolve_expr(false_expr);
            }
            ExprKind::Match { value, arms } => {
                self.resolve_expr(value);
                for arm in arms {
                    self.scopes.push_scope();

                    match &arm.pattern {
                        ast::Pattern::Wildcard => {}
                        ast::Pattern::Variant { path, bindings } => {
                            if !path.is_empty() && !self.scopes.resolve(path[0]) {
                                self.error(
                                    expr.span,
                                    DiagnosticCode::UnknownIdentifier,
                                    &format!(
                                        "Cannot find '{}' in this scope.",
                                        self.session.interner.borrow().lookup(path[0])
                                    ),
                                );
                            }
                            if let Some(binds) = bindings {
                                for bind in binds {
                                    if *bind != self.session.interner.borrow_mut().intern("_") {
                                        self.scopes.declare(*bind);
                                    }
                                }
                            }
                        }
                    }

                    self.resolve_expr(arm.body);
                    self.scopes.pop_scope();
                }
            }
        }
    }

    fn error(&mut self, span: Span, code: DiagnosticCode, message: &str) {
        self.errors
            .push(DiagnosticBuilder::error(code, message, span).build());
    }
}

#[cfg(any())]
mod tests {
    use super::*;
    use ast::{ExprKind, Location, StmtKind};

    fn make_span() -> Span {
        Span::new(0, 0, 0, Location::new(1, 1), Location::new(1, 1))
    }

    #[test]
    fn test_valid_shadowing() {
        // let x = 1; { let x = 2; print(x); }
        let outer_let = Stmt::new(
            StmtKind::Let {
                name: session.interner.borrow_mut().intern("x"),
                is_private: false,
                type_annotation: None,
                initializer: Some(Expr::new(ExprKind::Integer(1), make_span())),
            },
            make_span(),
        );

        let inner_let = Stmt::new(
            StmtKind::Let {
                name: session.interner.borrow_mut().intern("x"),
                is_private: false,
                type_annotation: None,
                initializer: Some(Expr::new(ExprKind::Integer(2), make_span())),
            },
            make_span(),
        );

        let inner_usage = Stmt::new(
            StmtKind::Expression(Expr::new(
                ExprKind::Variable(session.interner.borrow_mut().intern("x")),
                make_span(),
            )),
            make_span(),
        );

        let block = Stmt::new(StmtKind::Block(vec![inner_let, inner_usage]), make_span());

        let mut session = session::CompilerSession::new();
        let mut resolver = Resolver::new(&mut session);
        resolver.resolve(&[outer_let, block]);

        assert!(
            resolver.errors.is_empty(),
            "Expected no errors, got: {:?}",
            resolver.errors
        );
    }

    #[test]
    fn test_invalid_redeclaration() {
        // let x = 1; let x = 2;
        let let1 = Stmt::new(
            StmtKind::Let {
                name: session.interner.borrow_mut().intern("x"),
                is_private: false,
                type_annotation: None,
                initializer: Some(Expr::new(ExprKind::Integer(1), make_span())),
            },
            make_span(),
        );

        let let2 = Stmt::new(
            StmtKind::Let {
                name: session.interner.borrow_mut().intern("x"),
                is_private: false,
                type_annotation: None,
                initializer: Some(Expr::new(ExprKind::Integer(2), make_span())),
            },
            make_span(),
        );

        let mut session = session::CompilerSession::new();
        let mut resolver = Resolver::new(&mut session);
        resolver.resolve(&[let1, let2]);

        assert_eq!(resolver.errors.len(), 1);
        assert!(resolver.errors[0].message.contains("already declared"));
    }

    #[test]
    fn test_undefined_variable() {
        // print(y);
        let usage = Stmt::new(
            StmtKind::Expression(Expr::new(
                ExprKind::Variable(session.interner.borrow_mut().intern("y")),
                make_span(),
            )),
            make_span(),
        );

        let mut session = session::CompilerSession::new();
        let mut resolver = Resolver::new(&mut session);
        resolver.resolve(&[usage]);

        assert_eq!(resolver.errors.len(), 1);
        assert!(
            resolver.errors[0]
                .message
                .contains("Cannot find variable 'y'")
        );
    }
}
