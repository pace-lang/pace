pub mod declarations;
pub mod control_flow;

use super::*;

impl<'a> TypeChecker<'a> {
    pub(crate) fn check_stmt(&mut self, stmt: &Stmt<'a>) -> TypedStmt<'a> {
        let kind = match &stmt.kind {
            StmtKind::Block(stmts) => {
                self.env.push_scope();
                self.collect_declarations(stmts);
                let typed_stmts = self.check(stmts);
                self.env.pop_scope();
                TypedStmtKind::Block(typed_stmts)
            }
            StmtKind::Binding {
                name,
                type_annotation,
                initializer,
                mutability,
                is_weak,
                is_private: _,
                is_static,
            } => self.check_var_decl(*name, type_annotation, initializer, *is_weak, *mutability, *is_static, stmt.span).kind,
            StmtKind::Class {
                name,
                type_params,
                implements,
                methods,
                fields,
                is_private,
            } => self.check_class_decl(name, type_params, implements, methods, fields, *is_private, false, stmt.span),
            StmtKind::Actor {
                name,
                type_params,
                implements,
                methods,
                fields,
                is_private,
            } => self.check_class_decl(name, type_params, implements, methods, fields, *is_private, true, stmt.span),
            StmtKind::Struct {
                name,
                type_params,
                methods,
                fields,
                is_private,
            } => self.check_struct_decl(name, type_params, methods, fields, *is_private, stmt.span),
            StmtKind::Interface {
                name,
                type_params,
                methods: _,
                is_private: _,
            } => TypedStmtKind::Interface {
                name: *name,
                type_params: type_params.clone(),
                methods: Vec::new(),
            },
            StmtKind::Enum {
                name,
                type_params,
                variants,
                methods,
                is_private,
            } => self.check_enum_decl(name, type_params, variants, methods, *is_private),
            StmtKind::TypeAlias {
                name,
                type_params,
                target_type,
                is_private: _,
            } => TypedStmtKind::TypeAlias {
                name: *name,
                type_params: type_params.clone(),
                target_type: target_type.clone(),
            },
            StmtKind::ForeignFunc {
                name,
                base_name,
                type_params: _,
                params,
                return_type,
                is_private: _,
                is_static,
            } => TypedStmtKind::ForeignFunc {
                name: *name,
                base_name: *base_name,
                params: params.clone(),
                return_type: return_type.clone(),
                is_static: *is_static,
            },
            StmtKind::Func {
                name,
                type_params,
                params,
                return_type,
                body,
                is_private,
                is_async,
                is_static,
            } => self.check_func_decl(name, type_params, params, return_type, body, *is_private, *is_async, *is_static, stmt.span),
            StmtKind::If {
                condition,
                then_branch,
                else_branch,
            } => self.check_if_stmt(condition, then_branch, else_branch),
            StmtKind::While { condition, body } => self.check_while_stmt(condition, body),
            StmtKind::For {
                item_name,
                iterator,
                body,
            } => self.check_for_stmt(item_name, iterator, body, stmt.span),
            StmtKind::Extension {
                target_type,
                type_params,
                methods,
            } => self.check_extension_decl(target_type, type_params, methods, stmt.span),
            StmtKind::Import { .. } | StmtKind::Export { .. } => TypedStmtKind::Block(vec![]),
            StmtKind::Expression(expr) => TypedStmtKind::Expression({
                let tmp = self.check_expr(expr);
                self.alloc(tmp)
            }),
            StmtKind::Return { value } => self.check_return_stmt(value, stmt.span),
        };
        TypedStmt::new(kind, stmt.span)
    }
}
