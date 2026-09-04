use crate::{
    Expr, Stmt,
    arena::{AstArena, ExprId, StmtId},
};

impl AstArena {
    pub fn deep_clone_expr(&mut self, expr_id: ExprId) -> ExprId {
        let span = self.get_expr_span(expr_id);
        let expr = self.get_expr(expr_id).clone();
        let cloned_expr = match expr {
            Expr::InterpolatedString(parts) => {
                let new_parts = parts.into_iter().map(|e| self.deep_clone_expr(e)).collect();
                Expr::InterpolatedString(new_parts)
            }
            Expr::GenericInstantiation {
                callee,
                generic_args,
            } => {
                let new_callee = self.deep_clone_expr(callee);
                Expr::GenericInstantiation {
                    callee: new_callee,
                    generic_args,
                }
            }
            Expr::Binary { left, op, right } => {
                let new_left = self.deep_clone_expr(left);
                let new_right = self.deep_clone_expr(right);
                Expr::Binary {
                    left: new_left,
                    op,
                    right: new_right,
                }
            }
            Expr::Call { callee, args } => {
                let new_callee = self.deep_clone_expr(callee);
                let new_args = args
                    .into_iter()
                    .map(|arg| self.deep_clone_expr(arg))
                    .collect();
                Expr::Call {
                    callee: new_callee,
                    args: new_args,
                }
            }
            Expr::Assign { target, value } => {
                let new_target = self.deep_clone_expr(target);
                let new_value = self.deep_clone_expr(value);
                Expr::Assign {
                    target: new_target,
                    value: new_value,
                }
            }
            Expr::MemberAccess {
                object,
                property,
                computed_class,
                is_static_operator,
            } => {
                let new_object = self.deep_clone_expr(object);
                Expr::MemberAccess {
                    object: new_object,
                    property,
                    computed_class,
                    is_static_operator,
                }
            }
            Expr::Unwrap(expr) => Expr::Unwrap(self.deep_clone_expr(expr)),
            Expr::OptionalMemberAccess { object, property } => {
                let new_object = self.deep_clone_expr(object);
                Expr::OptionalMemberAccess {
                    object: new_object,
                    property,
                }
            }
            Expr::NullCoalesce { left, right } => {
                let new_left = self.deep_clone_expr(left);
                let new_right = self.deep_clone_expr(right);
                Expr::NullCoalesce {
                    left: new_left,
                    right: new_right,
                }
            }
            Expr::Try(expr) => Expr::Try(self.deep_clone_expr(expr)),
            Expr::Await(expr) => Expr::Await(self.deep_clone_expr(expr)),
            Expr::Closure {
                params,
                return_type,
                body,
            } => {
                let new_body = self.deep_clone_expr(body);
                Expr::Closure {
                    params,
                    return_type,
                    body: new_body,
                }
            }
            Expr::Block(stmts) => {
                let new_stmts = stmts.into_iter().map(|s| self.deep_clone_stmt(s)).collect();
                Expr::Block(new_stmts)
            }
            // Leaf nodes
            other => other,
        };
        self.alloc_expr(cloned_expr, span)
    }

    pub fn deep_clone_stmt(&mut self, stmt_id: StmtId) -> StmtId {
        let span = self.get_stmt_span(stmt_id);
        let stmt = self.get_stmt(stmt_id).clone();
        let cloned_stmt = match stmt {
            Stmt::Expr(expr) => Stmt::Expr(self.deep_clone_expr(expr)),
            Stmt::VarDecl {
                name,
                is_mutable,
                type_annotation,
                is_static,
                is_weak,
                visibility,
                initializer,
                span,
            } => {
                let new_init = initializer.map(|e| self.deep_clone_expr(e));
                Stmt::VarDecl {
                    name,
                    is_mutable,
                    type_annotation,
                    is_static,
                    is_weak,
                    visibility,
                    initializer: new_init,
                    span,
                }
            }
            Stmt::Block(stmts) => {
                let new_stmts = stmts.into_iter().map(|s| self.deep_clone_stmt(s)).collect();
                Stmt::Block(new_stmts)
            }
            Stmt::Return(expr) => Stmt::Return(expr.map(|e| self.deep_clone_expr(e))),
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let new_cond = self.deep_clone_expr(condition);
                let new_then = self.deep_clone_stmt(then_branch);
                let new_else = else_branch.map(|eb| self.deep_clone_stmt(eb));
                Stmt::If {
                    condition: new_cond,
                    then_branch: new_then,
                    else_branch: new_else,
                }
            }
            Stmt::While { condition, body } => {
                let new_cond = self.deep_clone_expr(condition);
                let new_body = self.deep_clone_stmt(body);
                Stmt::While {
                    condition: new_cond,
                    body: new_body,
                }
            }
            Stmt::Loop { body } => {
                let new_body = self.deep_clone_stmt(body);
                Stmt::Loop { body: new_body }
            }
            Stmt::ForIn {
                item,
                iterable,
                body,
            } => {
                let new_iter = self.deep_clone_expr(iterable);
                let new_body = self.deep_clone_stmt(body);
                Stmt::ForIn {
                    item,
                    iterable: new_iter,
                    body: new_body,
                }
            }
            Stmt::FuncDecl {
                name,
                generic_params,
                params,
                return_type,
                body,
                is_async,
                is_static,
                is_extern,
                visibility,
                span,
            } => {
                let new_body = body.into_iter().map(|s| self.deep_clone_stmt(s)).collect();
                Stmt::FuncDecl {
                    name,
                    generic_params,
                    params,
                    return_type,
                    body: new_body,
                    is_async,
                    is_static,
                    is_extern,
                    visibility,
                    span,
                }
            }
            Stmt::ClassDecl {
                name,
                generic_params,
                fields,
                methods,
                implements,
                visibility,
            } => {
                let new_fields = fields
                    .into_iter()
                    .map(|f| self.deep_clone_stmt(f))
                    .collect();
                let new_methods = methods
                    .into_iter()
                    .map(|m| self.deep_clone_stmt(m))
                    .collect();
                Stmt::ClassDecl {
                    name,
                    generic_params,
                    fields: new_fields,
                    methods: new_methods,
                    implements,
                    visibility,
                }
            }
            Stmt::ActorDecl {
                name,
                generic_params,
                fields,
                methods,
                implements,
                visibility,
            } => {
                let new_fields = fields
                    .into_iter()
                    .map(|f| self.deep_clone_stmt(f))
                    .collect();
                let new_methods = methods
                    .into_iter()
                    .map(|m| self.deep_clone_stmt(m))
                    .collect();
                Stmt::ActorDecl {
                    name,
                    generic_params,
                    fields: new_fields,
                    methods: new_methods,
                    implements,
                    visibility,
                }
            }
            Stmt::InterfaceDecl {
                name,
                generic_params,
                methods,
                visibility,
            } => {
                let new_methods = methods
                    .into_iter()
                    .map(|m| self.deep_clone_stmt(m))
                    .collect();
                Stmt::InterfaceDecl {
                    name,
                    generic_params,
                    methods: new_methods,
                    visibility,
                }
            }
            Stmt::StructDecl {
                name,
                generic_params,
                fields,
                visibility,
            } => {
                let new_fields = fields
                    .into_iter()
                    .map(|f| self.deep_clone_stmt(f))
                    .collect();
                Stmt::StructDecl {
                    name,
                    generic_params,
                    fields: new_fields,
                    visibility,
                }
            }
            Stmt::Module { name, body } => {
                let new_body = body.into_iter().map(|s| self.deep_clone_stmt(s)).collect();
                Stmt::Module {
                    name,
                    body: new_body,
                }
            }
            other => other,
        };
        self.alloc_stmt(cloned_stmt, span)
    }
}
