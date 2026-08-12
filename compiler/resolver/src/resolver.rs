use module::graph::ModuleGraph;
use module::module::Module;
use module::module_id::ModuleId;
use std::collections::HashMap;

use ast::{Expr, ExprKind, Stmt, StmtKind, Span};
use diagnostics::{Diagnostic, DiagnosticBuilder, DiagnosticCode};
use crate::scope::ScopeStack;

pub struct Resolver {
    scopes: ScopeStack,
    pub errors: Vec<Diagnostic>,
    // module_exports maps a ModuleId to a map of exported names and their sub-exports (e.g. enum variants)
    module_exports: HashMap<ModuleId, HashMap<String, Vec<String>>>,
}

impl Resolver {
    pub fn new() -> Self {
        let mut scopes = ScopeStack::new();
        scopes.declare("print".into());
        scopes.declare("Result".into());
        scopes.declare("Ok".into());
        scopes.declare("Err".into());
        scopes.declare("Option".into());
        scopes.declare("Some".into());
        scopes.declare("None".into());
        Self {
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
                    StmtKind::Let { name, is_private: false, .. } |
                    StmtKind::Var { name, is_private: false, .. } |
                    StmtKind::Class { name, is_private: false, .. } |
                    StmtKind::Interface { name, is_private: false, .. } |
                    StmtKind::ForeignFunc { name, is_private: false, .. } |
                    StmtKind::Func { name, is_private: false, .. } => {
                        exports.insert(name.clone(), Vec::new());
                    }
                    StmtKind::Enum { name, is_private: false, variants, .. } => {
                        let mut sub_exports = Vec::new();
                        for variant in variants {
                            sub_exports.push(variant.name.clone());
                        }
                        exports.insert(name.clone(), sub_exports);
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
            if let StmtKind::Import { path, alias, show, hide } = &stmt.kind {
                // Find the imported module
                let clean_path = path.trim_matches('"').trim_matches('\'');
                let imported_id = graph.resolve_import(module.id, clean_path);

                if let Some(id) = imported_id {
                    if let Some(exports) = self.module_exports.get(&id) {
                        if let Some(alias_name) = alias {
                            // alias imports act as a namespace, but for now we just declare the alias
                            // In a real implementation, we'd bind it to a module object.
                            self.scopes.declare(alias_name.clone());
                        } else {
                            for (export, sub_exports) in exports {
                                if (!show.is_empty() && !show.contains(export)) || hide.contains(export) {
                                    continue;
                                }
                                self.scopes.declare(export.clone());
                                for sub in sub_exports {
                                    self.scopes.declare(sub.clone());
                                }
                            }
                        }
                    }
                } else {
                    self.error(stmt.span.clone(), DiagnosticCode::UnknownIdentifier, &format!("Cannot resolve import '{}'", path));
                }
            }
        }

        self.resolve(&module.ast);
        self.scopes.pop_scope();
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
            StmtKind::Let { name, type_annotation: _, initializer, is_private: _ } | StmtKind::Var { name, type_annotation: _, initializer, is_weak: _, is_private: _ } => {
                // Resolve initializer first so it can't reference the variable being declared
                if let Some(init) = initializer {
                    self.resolve_expr(init);
                }
                
                if !self.scopes.declare(name.clone()) {
                    self.error(stmt.span, DiagnosticCode::DuplicateDeclaration, &format!("Variable '{}' is already declared in this scope.", name));
                }
            }
            StmtKind::Class { name, type_params: _, implements: _, methods, fields, is_private: _ } => {
                if !self.scopes.declare(name.clone()) {
                    self.error(stmt.span, DiagnosticCode::DuplicateDeclaration, &format!("Class '{}' is already declared in this scope.", name));
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
            StmtKind::Interface { name, methods: _methods, is_private: _ } => {
                if !self.scopes.declare(name.clone()) {
                    self.error(stmt.span, DiagnosticCode::DuplicateDeclaration, &format!("Interface '{}' is already declared in this scope.", name));
                }
            }
            StmtKind::Enum { name, type_params: _, variants: _, is_private: _ } => {
                if !self.scopes.declare(name.clone()) {
                    self.error(stmt.span, DiagnosticCode::DuplicateDeclaration, &format!("Enum '{}' is already declared in this scope.", name));
                }
            }
            StmtKind::ForeignFunc { name, type_params: _, params: _, return_type: _, is_private: _ } => {
                if !self.scopes.declare(name.clone()) {
                    self.error(stmt.span, DiagnosticCode::DuplicateDeclaration, &format!("Foreign function '{}' is already declared in this scope.", name));
                }
            }
            StmtKind::Func { name, type_params: _, params, return_type: _, body, is_private: _ } => {
                self.scopes.declare(name.clone());

                self.scopes.push_scope();
                for (param_name, _) in params {
                    if !self.scopes.declare(param_name.clone()) {
                        self.error(stmt.span, DiagnosticCode::DuplicateDeclaration, &format!("Parameter '{}' is declared multiple times.", param_name));
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
            StmtKind::Import { .. } | StmtKind::Export { .. } => {},
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
                    self.error(expr.span, DiagnosticCode::UnknownIdentifier, &format!("Cannot find variable '{}' in this scope.", name));
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
            ExprKind::Call { callee, type_args: _, arguments } => {
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
                    self.error(expr.span, DiagnosticCode::UnknownIdentifier, &format!("Cannot assign to undefined variable '{}'.", name));
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
            ExprKind::IndexSet { object, index, value } => {
                self.resolve_expr(object);
                self.resolve_expr(index);
                self.resolve_expr(value);
            }
            ExprKind::SelfRef => {
                if !self.scopes.resolve(&"self".to_string()) {
                    self.error(expr.span, DiagnosticCode::UnknownIdentifier, "Cannot use 'self' outside of a class method.");
                }
            }
            // Literals have no names to resolve
            ExprKind::Integer(_) | ExprKind::Float(_) | ExprKind::String(_) | ExprKind::Boolean(_) | ExprKind::Null => {}
            ExprKind::ForceUnwrap(inner) => {
                self.resolve_expr(inner);
            }
            ExprKind::OptionalGet { object, name: _ } => {
                self.resolve_expr(object);
            }
            ExprKind::Match { value, arms } => {
                self.resolve_expr(value);
                for arm in arms {
                    self.scopes.push_scope();
                    
                    match &arm.pattern {
                        ast::Pattern::Wildcard => {}
                        ast::Pattern::Variant { path, bindings } => {
                            if !path.is_empty() {
                                if !self.scopes.resolve(&path[0]) {
                                    self.error(expr.span, DiagnosticCode::UnknownIdentifier, &format!("Cannot find '{}' in this scope.", path[0]));
                                }
                            }
                            if let Some(binds) = bindings {
                                for bind in binds {
                                    if bind != "_" {
                                        self.scopes.declare(bind.clone());
                                    }
                                }
                            }
                        }
                    }

                    self.resolve_expr(&arm.body);
                    self.scopes.pop_scope();
                }
            }
        }
    }

    fn error(&mut self, span: Span, code: DiagnosticCode, message: &str) {
        self.errors.push(DiagnosticBuilder::error(code, message, span).build());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ast::{ExprKind, StmtKind, Location};

    fn make_span() -> Span {
        Span::new(0, 0, 0, Location::new(1, 1), Location::new(1, 1))
    }

    #[test]
    fn test_valid_shadowing() {
        // let x = 1; { let x = 2; print(x); }
        let outer_let = Stmt::new(StmtKind::Let {
            name: "x".into(),
            is_private: false,
            type_annotation: None,
            initializer: Some(Expr::new(ExprKind::Integer(1), make_span())),
        }, make_span());

        let inner_let = Stmt::new(StmtKind::Let {
            name: "x".into(),
            is_private: false,
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
            is_private: false,
            type_annotation: None,
            initializer: Some(Expr::new(ExprKind::Integer(1), make_span())),
        }, make_span());

        let let2 = Stmt::new(StmtKind::Let {
            name: "x".into(),
            is_private: false,
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
