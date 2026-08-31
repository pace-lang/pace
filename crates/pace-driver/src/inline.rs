use pace_ast::{Expr, Stmt};
use std::collections::HashMap;

pub struct Inliner {
    // Map from function name to (parameter names, body expression)
    inlineable: HashMap<ustr::Ustr, (Vec<ustr::Ustr>, pace_ast::arena::ExprId)>,
}

impl Default for Inliner {
    fn default() -> Self {
        Self::new()
    }
}

impl Inliner {
    pub fn new() -> Self {
        Self {
            inlineable: HashMap::new(),
        }
    }

    pub fn run(
        arena: &mut pace_ast::arena::AstArena,
        ast: Vec<pace_ast::arena::StmtId>,
    ) -> Vec<pace_ast::arena::StmtId> {
        let mut inliner = Self::new();
        inliner.extract_inlineable(arena, &ast);

        let mut new_ast = Vec::new();
        for stmt_id in ast {
            new_ast.push(inliner.rewrite_stmt(arena, stmt_id));
        }
        new_ast
    }

    fn extract_inlineable(
        &mut self,
        arena: &pace_ast::arena::AstArena,
        ast: &[pace_ast::arena::StmtId],
    ) {
        for &stmt_id in ast {
            let stmt = arena.get_stmt(stmt_id);
            match stmt {
                Stmt::Module { body, .. } => {
                    self.extract_inlineable(arena, body);
                }
                Stmt::FuncDecl {
                    name, params, body, ..
                } => {
                    if let Some(expr) = self.get_single_expression(arena, body) {
                        let param_names = params.iter().map(|p| p.name).collect();
                        self.inlineable.insert(*name, (param_names, expr));
                    }
                }
                _ => {}
            }
        }
    }

    fn get_single_expression(
        &self,
        arena: &pace_ast::arena::AstArena,
        body: &[pace_ast::arena::StmtId],
    ) -> Option<pace_ast::arena::ExprId> {
        if body.len() == 1 {
            match arena.get_stmt(body[0]) {
                Stmt::Return(Some(expr)) => Some(*expr),
                Stmt::Expr(expr) => Some(*expr),
                _ => None,
            }
        } else {
            None
        }
    }

    fn rewrite_stmt(
        &mut self,
        arena: &mut pace_ast::arena::AstArena,
        stmt_id: pace_ast::arena::StmtId,
    ) -> pace_ast::arena::StmtId {
        let stmt = arena.get_stmt(stmt_id).clone();
        let new_stmt = match stmt {
            Stmt::Module { name, body } => {
                let new_body = body
                    .into_iter()
                    .map(|s| self.rewrite_stmt(arena, s))
                    .collect();
                Stmt::Module {
                    name,
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
                let new_body = body
                    .into_iter()
                    .map(|s| self.rewrite_stmt(arena, s))
                    .collect();
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
            } => {
                let new_fields = fields
                    .into_iter()
                    .map(|f| self.rewrite_stmt(arena, f))
                    .collect();
                let new_methods = methods
                    .into_iter()
                    .map(|m| self.rewrite_stmt(arena, m))
                    .collect();
                Stmt::ClassDecl {
                    name,
                    generic_params,
                    fields: new_fields,
                    methods: new_methods,
                    implements,
                }
            }
            Stmt::ActorDecl {
                name,
                generic_params,
                fields,
                methods,
                implements,
            } => {
                let new_fields = fields
                    .into_iter()
                    .map(|f| self.rewrite_stmt(arena, f))
                    .collect();
                let new_methods = methods
                    .into_iter()
                    .map(|m| self.rewrite_stmt(arena, m))
                    .collect();
                Stmt::ActorDecl {
                    name,
                    generic_params,
                    fields: new_fields,
                    methods: new_methods,
                    implements,
                }
            }
            Stmt::VarDecl {
                name,
                is_mutable,
                type_annotation,
                is_static,
                visibility,
                initializer,
                span,
            } => {
                let new_initializer = initializer.map(|expr| self.rewrite_expr(arena, expr));
                Stmt::VarDecl {
                    name,
                    is_mutable,
                    type_annotation,
                    is_static,
                    visibility,
                    initializer: new_initializer,
                    span,
                }
            }
            Stmt::Expr(expr) => Stmt::Expr(self.rewrite_expr(arena, expr)),
            Stmt::Return(expr) => Stmt::Return(expr.map(|e| self.rewrite_expr(arena, e))),
            Stmt::Block(stmts) => Stmt::Block(
                stmts
                    .into_iter()
                    .map(|s| self.rewrite_stmt(arena, s))
                    .collect(),
            ),
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => Stmt::If {
                condition: self.rewrite_expr(arena, condition),
                then_branch: self.rewrite_stmt(arena, then_branch),
                else_branch: else_branch.map(|eb| self.rewrite_stmt(arena, eb)),
            },
            Stmt::While { condition, body } => Stmt::While {
                condition: self.rewrite_expr(arena, condition),
                body: self.rewrite_stmt(arena, body),
            },
            Stmt::Match { expr, arms } => {
                let new_arms = arms
                    .into_iter()
                    .map(|(pat, body)| (pat, self.rewrite_stmt(arena, body)))
                    .collect();
                Stmt::Match {
                    expr: self.rewrite_expr(arena, expr),
                    arms: new_arms,
                }
            }
            Stmt::ForIn {
                item,
                iterable,
                body,
            } => Stmt::ForIn {
                item,
                iterable: self.rewrite_expr(arena, iterable),
                body: self.rewrite_stmt(arena, body),
            },
            _ => stmt,
        };
        *arena.get_stmt_mut(stmt_id) = new_stmt;
        stmt_id
    }

    fn rewrite_expr(
        &mut self,
        arena: &mut pace_ast::arena::AstArena,
        expr_id: pace_ast::arena::ExprId,
    ) -> pace_ast::arena::ExprId {
        let expr = arena.get_expr(expr_id).clone();
        let new_expr = match expr {
            Expr::Call { callee, args } => {
                let new_callee = self.rewrite_expr(arena, callee);
                let new_args: Vec<pace_ast::arena::ExprId> = args
                    .into_iter()
                    .map(|a| self.rewrite_expr(arena, a))
                    .collect();

                if let Expr::Identifier(name, _) = arena.get_expr(new_callee)
                    && let Some((param_names, body_expr)) = self.inlineable.get(name).cloned()
                {
                    // Check if all arguments are safe to inline (no side effects)
                    if new_args
                        .iter()
                        .all(|arg| self.is_safe_argument(arena, *arg))
                        && new_args.len() == param_names.len()
                    {
                        return self.inline_call(arena, body_expr, &param_names, &new_args);
                    }
                }

                Expr::Call {
                    callee: new_callee,
                    args: new_args,
                }
            }
            Expr::Binary { left, op, right } => Expr::Binary {
                left: self.rewrite_expr(arena, left),
                op,
                right: self.rewrite_expr(arena, right),
            },
            Expr::Assign { target, value } => Expr::Assign {
                target: self.rewrite_expr(arena, target),
                value: self.rewrite_expr(arena, value),
            },
            Expr::MemberAccess {
                object,
                property,
                computed_class,
                is_static_operator,
            } => Expr::MemberAccess {
                object: self.rewrite_expr(arena, object),
                property,
                computed_class,
                is_static_operator,
            },
            Expr::OptionalMemberAccess { object, property } => Expr::OptionalMemberAccess {
                object: self.rewrite_expr(arena, object),
                property,
            },
            Expr::Try(inner) => Expr::Try(self.rewrite_expr(arena, inner)),
            Expr::Unwrap(inner) => Expr::Unwrap(self.rewrite_expr(arena, inner)),
            Expr::Await(inner) => Expr::Await(self.rewrite_expr(arena, inner)),
            Expr::GenericInstantiation {
                callee,
                generic_args,
            } => Expr::GenericInstantiation {
                callee: self.rewrite_expr(arena, callee),
                generic_args,
            },
            Expr::InterpolatedString(parts) => Expr::InterpolatedString(
                parts
                    .into_iter()
                    .map(|p| self.rewrite_expr(arena, p))
                    .collect(),
            ),
            Expr::NullCoalesce { left, right } => Expr::NullCoalesce {
                left: self.rewrite_expr(arena, left),
                right: self.rewrite_expr(arena, right),
            },
            Expr::Block(stmts) => Expr::Block(
                stmts
                    .into_iter()
                    .map(|s| self.rewrite_stmt(arena, s))
                    .collect(),
            ),
            Expr::Closure {
                params,
                return_type,
                body,
            } => Expr::Closure {
                params,
                return_type,
                body: self.rewrite_expr(arena, body),
            },
            _ => expr,
        };
        *arena.get_expr_mut(expr_id) = new_expr;
        expr_id
    }

    fn is_safe_argument(
        &self,
        arena: &pace_ast::arena::AstArena,
        arg: pace_ast::arena::ExprId,
    ) -> bool {
        let arg = arena.get_expr(arg);
        matches!(
            arg,
            Expr::IntLiteral(_)
                | Expr::FloatLiteral(_)
                | Expr::StringLiteral(_)
                | Expr::BoolLiteral(_)
                | Expr::Null
                | Expr::Identifier(_, _)
        )
    }

    fn inline_call(
        &mut self,
        arena: &mut pace_ast::arena::AstArena,
        body: pace_ast::arena::ExprId,
        param_names: &[ustr::Ustr],
        args: &[pace_ast::arena::ExprId],
    ) -> pace_ast::arena::ExprId {
        // Build a substitution map
        let mut subs = HashMap::new();
        for (i, param_name) in param_names.iter().enumerate() {
            subs.insert(*param_name, args[i]);
        }
        let body_clone = arena.deep_clone_expr(body);
        self.substitute_expr(arena, body_clone, &subs);
        // After substituting, rewrite the inlined expression again in case it contains more calls
        self.rewrite_expr(arena, body_clone)
    }

    fn substitute_expr(
        &self,
        arena: &mut pace_ast::arena::AstArena,
        expr_id: pace_ast::arena::ExprId,
        subs: &HashMap<ustr::Ustr, pace_ast::arena::ExprId>,
    ) {
        let mut expr = arena.get_expr(expr_id).clone();
        match &mut expr {
            Expr::Identifier(name, _) => {
                if let Some(sub) = subs.get(name) {
                    let cloned_sub = arena.deep_clone_expr(*sub);
                    *arena.get_expr_mut(expr_id) = arena.get_expr(cloned_sub).clone();
                    return;
                }
            }
            Expr::Call { callee, args } => {
                self.substitute_expr(arena, *callee, subs);
                for arg in args {
                    self.substitute_expr(arena, *arg, subs);
                }
            }
            Expr::Binary { left, right, .. } => {
                self.substitute_expr(arena, *left, subs);
                self.substitute_expr(arena, *right, subs);
            }
            Expr::Assign { target, value } => {
                self.substitute_expr(arena, *target, subs);
                self.substitute_expr(arena, *value, subs);
            }
            Expr::MemberAccess { object, .. } => {
                self.substitute_expr(arena, *object, subs);
            }
            Expr::OptionalMemberAccess { object, .. } => {
                self.substitute_expr(arena, *object, subs);
            }
            Expr::Try(inner) | Expr::Unwrap(inner) | Expr::Await(inner) => {
                self.substitute_expr(arena, *inner, subs);
            }
            Expr::GenericInstantiation { callee, .. } => {
                self.substitute_expr(arena, *callee, subs);
            }
            Expr::InterpolatedString(parts) => {
                for part in parts {
                    self.substitute_expr(arena, *part, subs);
                }
            }
            Expr::NullCoalesce { left, right } => {
                self.substitute_expr(arena, *left, subs);
                self.substitute_expr(arena, *right, subs);
            }
            Expr::Block(stmts) => {
                for stmt in stmts {
                    self.substitute_stmt(arena, *stmt, subs);
                }
            }
            Expr::Closure { body, .. } => {
                self.substitute_expr(arena, *body, subs);
            }
            _ => {}
        }
        *arena.get_expr_mut(expr_id) = expr;
    }

    fn substitute_stmt(
        &self,
        arena: &mut pace_ast::arena::AstArena,
        stmt_id: pace_ast::arena::StmtId,
        subs: &std::collections::HashMap<ustr::Ustr, pace_ast::arena::ExprId>,
    ) {
        let mut stmt = arena.get_stmt(stmt_id).clone();
        match &mut stmt {
            Stmt::Expr(expr) => self.substitute_expr(arena, *expr, subs),
            Stmt::Return(Some(expr)) => self.substitute_expr(arena, *expr, subs),
            Stmt::VarDecl {
                initializer: Some(expr),
                ..
            } => self.substitute_expr(arena, *expr, subs),
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.substitute_expr(arena, *condition, subs);
                self.substitute_stmt(arena, *then_branch, subs);
                if let Some(eb) = else_branch {
                    self.substitute_stmt(arena, *eb, subs);
                }
            }
            Stmt::While { condition, body } => {
                self.substitute_expr(arena, *condition, subs);
                self.substitute_stmt(arena, *body, subs);
            }
            Stmt::Block(stmts) => {
                for s in stmts {
                    self.substitute_stmt(arena, *s, subs);
                }
            }
            Stmt::Match { expr, arms } => {
                self.substitute_expr(arena, *expr, subs);
                for (_, body) in arms {
                    self.substitute_stmt(arena, *body, subs);
                }
            }
            Stmt::ForIn { iterable, body, .. } => {
                self.substitute_expr(arena, *iterable, subs);
                self.substitute_stmt(arena, *body, subs);
            }
            _ => {}
        }
        *arena.get_stmt_mut(stmt_id) = stmt;
    }
}
