pub mod classes;
pub mod types;
pub mod functions;

use super::*;

impl<'a> TypeChecker<'a> {
    pub fn collect_declarations(&mut self, statements: &[Stmt<'a>]) {
        for stmt in statements {
            match &stmt.kind {
                StmtKind::Block(_stmts) => {
                    // Do not eagerly collect inside blocks here, wait until check_stmt enters the block
                }
                StmtKind::Class {
                    name,
                    type_params,
                    implements,
                    methods,
                    fields,
                    is_private: _,
                } => self.hoist_class(stmt, name, type_params, implements, methods, fields, false),
                StmtKind::Actor {
                    name,
                    type_params,
                    implements,
                    methods,
                    fields,
                    is_private: _,
                } => self.hoist_class(stmt, name, type_params, implements, methods, fields, true),
                StmtKind::Struct {
                    name,
                    type_params,
                    methods,
                    fields,
                    is_private: _,
                } => self.hoist_struct(stmt, name, type_params, methods, fields),
                StmtKind::Interface {
                    name,
                    type_params,
                    methods,
                    is_private: _,
                } => self.hoist_interface(stmt, name, type_params, methods),
                StmtKind::Enum {
                    name,
                    type_params,
                    variants,
                    methods,
                    is_private: _,
                } => self.hoist_enum(stmt, name, type_params, variants, methods),
                StmtKind::TypeAlias {
                    name,
                    type_params,
                    target_type,
                    is_private: _,
                } => self.hoist_type_alias(stmt, name, type_params, target_type),
                StmtKind::ForeignFunc {
                    name,
                    base_name: _,
                    type_params,
                    params,
                    return_type,
                    is_private: _,
                    is_static: _,
                } => self.hoist_foreign_func(stmt, name, type_params, params, return_type),
                StmtKind::Func {
                    name,
                    type_params,
                    params,
                    return_type,
                    body: _,
                    is_private: _,
                    is_async,
                    is_static: _,
                } => self.hoist_func(stmt, name, type_params, params, return_type, *is_async),
                StmtKind::Extension {
                    target_type,
                    type_params,
                    methods,
                } => self.hoist_extension(stmt, target_type, type_params, methods),
                _ => {}
            }
        }
    }
}
