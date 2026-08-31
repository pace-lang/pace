use pace_ast::{BinaryOp, Expr, Stmt};

pub struct ConstantFolder {}

impl Default for ConstantFolder {
    fn default() -> Self {
        Self::new()
    }
}

impl ConstantFolder {
    pub fn new() -> Self {
        Self {}
    }

    pub fn run(
        arena: &mut pace_ast::arena::AstArena,
        ast: Vec<pace_ast::arena::StmtId>,
    ) -> Vec<pace_ast::arena::StmtId> {
        let mut folder = Self::new();
        let mut new_ast = Vec::new();
        for stmt_id in ast {
            new_ast.push(folder.rewrite_stmt(arena, stmt_id));
        }
        new_ast
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
            } => {
                let cond_expr = self.rewrite_expr(arena, condition);

                // Dead Code Elimination
                if let Expr::BoolLiteral(b) = arena.get_expr(cond_expr) {
                    if *b {
                        return self.rewrite_stmt(arena, then_branch);
                    } else if let Some(eb) = else_branch {
                        return self.rewrite_stmt(arena, eb);
                    } else {
                        // Dead branch, just return an empty block
                        return arena.alloc_stmt(Stmt::Block(vec![]));
                    }
                }

                Stmt::If {
                    condition: cond_expr,
                    then_branch: self.rewrite_stmt(arena, then_branch),
                    else_branch: else_branch.map(|eb| self.rewrite_stmt(arena, eb)),
                }
            }
            Stmt::While { condition, body } => {
                let cond_expr = self.rewrite_expr(arena, condition);

                if let Expr::BoolLiteral(b) = arena.get_expr(cond_expr)
                    && !*b {
                        return arena.alloc_stmt(Stmt::Block(vec![]));
                    }

                Stmt::While {
                    condition: cond_expr,
                    body: self.rewrite_stmt(arena, body),
                }
            }
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
                Expr::Call {
                    callee: new_callee,
                    args: new_args,
                }
            }
            Expr::Binary { left, op, right } => {
                let left_id = self.rewrite_expr(arena, left);
                let right_id = self.rewrite_expr(arena, right);

                let left_expr = arena.get_expr(left_id).clone();
                let right_expr = arena.get_expr(right_id).clone();

                // Constant Folding
                if let (Expr::IntLiteral(l), Expr::IntLiteral(r)) = (&left_expr, &right_expr) {
                    match op {
                        BinaryOp::Add => Expr::IntLiteral(l + r),
                        BinaryOp::Sub => Expr::IntLiteral(l - r),
                        BinaryOp::Mul => Expr::IntLiteral(l * r),
                        BinaryOp::Div if *r != 0 => Expr::IntLiteral(l / r),
                        BinaryOp::Mod if *r != 0 => Expr::IntLiteral(l % r),
                        BinaryOp::Eq => Expr::BoolLiteral(l == r),
                        BinaryOp::NotEq => Expr::BoolLiteral(l != r),
                        BinaryOp::Less => Expr::BoolLiteral(l < r),
                        BinaryOp::LessEq => Expr::BoolLiteral(l <= r),
                        BinaryOp::Greater => Expr::BoolLiteral(l > r),
                        BinaryOp::GreaterEq => Expr::BoolLiteral(l >= r),
                        _ => Expr::Binary {
                            left: left_id,
                            op,
                            right: right_id,
                        },
                    }
                } else if let (Expr::FloatLiteral(l), Expr::FloatLiteral(r)) =
                    (&left_expr, &right_expr)
                {
                    match op {
                        BinaryOp::Add => Expr::FloatLiteral(l + r),
                        BinaryOp::Sub => Expr::FloatLiteral(l - r),
                        BinaryOp::Mul => Expr::FloatLiteral(l * r),
                        BinaryOp::Div if *r != 0.0 => Expr::FloatLiteral(l / r),
                        BinaryOp::Mod if *r != 0.0 => Expr::FloatLiteral(l % r),
                        BinaryOp::Eq => Expr::BoolLiteral(l == r),
                        BinaryOp::NotEq => Expr::BoolLiteral(l != r),
                        BinaryOp::Less => Expr::BoolLiteral(l < r),
                        BinaryOp::LessEq => Expr::BoolLiteral(l <= r),
                        BinaryOp::Greater => Expr::BoolLiteral(l > r),
                        BinaryOp::GreaterEq => Expr::BoolLiteral(l >= r),
                        _ => Expr::Binary {
                            left: left_id,
                            op,
                            right: right_id,
                        },
                    }
                } else if let (Expr::BoolLiteral(l), Expr::BoolLiteral(r)) =
                    (&left_expr, &right_expr)
                {
                    match op {
                        BinaryOp::And => Expr::BoolLiteral(*l && *r),
                        BinaryOp::Or => Expr::BoolLiteral(*l || *r),
                        BinaryOp::Eq => Expr::BoolLiteral(l == r),
                        BinaryOp::NotEq => Expr::BoolLiteral(l != r),
                        _ => Expr::Binary {
                            left: left_id,
                            op,
                            right: right_id,
                        },
                    }
                } else {
                    Expr::Binary {
                        left: left_id,
                        op,
                        right: right_id,
                    }
                }
            }
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
}
