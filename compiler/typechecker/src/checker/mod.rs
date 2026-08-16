use crate::env::TypeEnvironment;
use ast::{
    BinaryOp, Expr, ExprKind, Span, Stmt, StmtKind, TypeExpr, TypedExpr, TypedExprKind, TypedStmt,
    TypedStmtKind, UnaryOp,
};
use module::graph::ModuleGraph;
use session::Symbol;
use session::TypeId;
use session::types::Type;
mod expr;
mod hoist;
mod stmt;
mod types_util;

#[cfg(test)]
mod tests;

use diagnostics::{Diagnostic, DiagnosticBuilder, DiagnosticCode};
use std::collections::HashMap;

pub struct TypeChecker<'a> {
    pub session: &'a session::CompilerSession,
    env: TypeEnvironment,
    pub errors: Vec<Diagnostic>,
    current_return_type: Option<TypeId>,
    pub classes: HashMap<Symbol, HashMap<Symbol, TypeId>>,
    pub class_mutables: HashMap<Symbol, HashMap<Symbol, bool>>,
    pub interfaces: HashMap<Symbol, HashMap<Symbol, TypeId>>,
    pub enums: HashMap<Symbol, HashMap<Symbol, TypeId>>,
    pub class_implements: HashMap<Symbol, Vec<TypeId>>,
    current_class: Option<Symbol>,
    pub generic_registry: generics::GenericDefinitionRegistry<'a>,
    pub spec_registry: generics::SpecializationRegistry,
    pub pending_instantiations: Vec<TypedStmt<'a>>,
    pub uninitialized_class_properties: HashMap<Symbol, Vec<Symbol>>,
    pub is_checking_method: bool,
}

impl<'a> TypeChecker<'a> {
    #[inline]
    pub fn get_type(&self, ty: TypeId) -> Type {
        self.session.types.borrow().get(ty).clone()
    }

    #[inline]
    pub fn arena(&self) -> &'a bumpalo::Bump {
        let session: &'a session::CompilerSession = self.session;
        &session.ast_arena
    }

    #[inline]
    pub fn alloc<T>(&self, val: T) -> &'a T {
        self.arena().alloc(val)
    }

    pub fn new(session: &'a session::CompilerSession) -> Self {
        let print_sym = session.interner.borrow_mut().intern("print");
        let builtin_func_ty = session.types.borrow_mut().intern(Type::BuiltinFunc);
        Self {
            session,
            env: TypeEnvironment::new(print_sym, builtin_func_ty),
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

    pub fn check_graph(&mut self, graph: &ModuleGraph<'a>) -> Vec<TypedStmt<'a>> {
        for module in graph.topological_sort() {
            self.collect_declarations(&module.ast);
        }

        let mut all_stmts = Vec::new();
        for module in graph.topological_sort() {
            let typed_ast = self.check(&module.ast);
            all_stmts.extend(typed_ast);
        }

        // Also drain pending generic instantiations
        let mut final_stmts = Vec::new();
        while !self.pending_instantiations.is_empty() {
            let pending: Vec<TypedStmt<'a>> = self.pending_instantiations.drain(..).collect();
            final_stmts.extend(pending);
        }

        final_stmts.extend(all_stmts);
        final_stmts
    }

    pub fn check_program(&mut self, statements: &[Stmt<'a>]) -> Vec<TypedStmt<'a>> {
        self.collect_declarations(statements);
        let typed_stmts = self.check(statements);

        let mut final_stmts = Vec::new();
        while !self.pending_instantiations.is_empty() {
            let pending: Vec<TypedStmt<'a>> = self.pending_instantiations.drain(..).collect();
            final_stmts.extend(pending);
        }

        final_stmts.extend(typed_stmts);
        final_stmts
    }

    pub fn check(&mut self, statements: &[Stmt<'a>]) -> Vec<TypedStmt<'a>> {
        let mut typed_stmts = Vec::new();
        for stmt in statements {
            typed_stmts.push(self.check_stmt(stmt));
        }
        typed_stmts
    }

    pub fn error(&mut self, span: Span, code: DiagnosticCode, message: &str) {
        self.errors
            .push(DiagnosticBuilder::error(code, message, span).build());
    }
}
