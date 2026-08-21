use super::super::*;
use session::types::{Type, TypeId};

impl<'a> TypeChecker<'a> {
    pub(crate) fn check_force_unwrap_expr(
        &mut self,
        inner: &Expr<'a>,
        span: Span,
    ) -> (TypedExprKind<'a>, TypeId) {
        let typed_inner = self.check_expr(inner);
        let ty = match self.get_type(typed_inner.ty) {
            Type::Optional(inner_inner) => inner_inner,
            Type::Null => {
                self.error(
                    span,
                    DiagnosticCode::TypeMismatch,
                    "Cannot force unwrap a null literal.",
                );
                self.session.types.borrow_mut().intern(Type::Error)
            }
            Type::Error | Type::Any => typed_inner.ty,
            _ => {
                self.error(
                    span,
                    DiagnosticCode::TypeMismatch,
                    &format!(
                        "Cannot force unwrap non-optional type '{}'.",
                        self.session.format_type(typed_inner.ty)
                    ),
                );
                typed_inner.ty
            }
        };
        (TypedExprKind::ForceUnwrap(self.alloc(typed_inner)), ty)
    }

    pub(crate) fn check_postfix_try_expr(
        &mut self,
        inner: &Expr<'a>,
        span: Span,
    ) -> (TypedExprKind<'a>, TypeId) {
        let typed_inner = self.check_expr(inner);
        let result_sym = self.session.interner.borrow_mut().intern("Result");
        let inner_ty = self.get_type(typed_inner.ty);

        let (t_ty, e_ty) = match inner_ty {
            Type::GenericInstance(sym, ref args) if sym == result_sym && args.len() == 2 => {
                (args[0], args[1])
            }
            Type::Error => {
                return (
                    TypedExprKind::PostfixTry(self.alloc(typed_inner)),
                    self.session.types.borrow_mut().intern(Type::Error),
                );
            }
            _ => {
                self.error(
                    span,
                    DiagnosticCode::TypeMismatch,
                    &format!(
                        "Cannot use `?` operator on type '{}', expected a Result type.",
                        self.session.format_type(typed_inner.ty)
                    ),
                );
                (
                    self.session.types.borrow_mut().intern(Type::Error),
                    self.session.types.borrow_mut().intern(Type::Error),
                )
            }
        };

        if t_ty != self.session.types.borrow_mut().intern(Type::Error) {
            if let Some(ret_id) = self.current_return_type {
                let ret_ty = self.get_type(ret_id);
                match ret_ty {
                    Type::GenericInstance(sym, ref args)
                        if sym == result_sym && args.len() == 2 =>
                    {
                        let func_e_ty = args[1];
                        if !self.is_assignable(e_ty, func_e_ty) {
                            self.error(
                                span,
                                DiagnosticCode::TypeMismatch,
                                &format!(
                                    "Cannot bubble up error of type '{}' into function returning error of type '{}'.",
                                    self.session.format_type(e_ty),
                                    self.session.format_type(func_e_ty)
                                ),
                            );
                        }
                    }
                    _ => {
                        self.error(
                            span,
                            DiagnosticCode::TypeMismatch,
                            &format!(
                                "Cannot use `?` operator in a function that returns '{}'. The function must return a Result.",
                                self.session.format_type(ret_id)
                            ),
                        );
                    }
                }
            } else {
                self.error(
                    span,
                    DiagnosticCode::TypeMismatch,
                    "Cannot use `?` operator outside of a function.",
                );
            }
        }

        (TypedExprKind::PostfixTry(self.alloc(typed_inner)), t_ty)
    }

    pub(crate) fn check_null_coalesce_expr(
        &mut self,
        left: &Expr<'a>,
        right: &Expr<'a>,
        span: Span,
    ) -> (TypedExprKind<'a>, TypeId) {
        let typed_left = self.check_expr(left);
        let typed_right = self.check_expr(right);

        match self.get_type(typed_left.ty) {
            Type::Optional(inner) => {
                let expected = inner;
                let is_valid = typed_right.ty == expected
                    || typed_right.ty == typed_left.ty
                    || typed_right.ty == self.session.types.borrow_mut().intern(Type::Error)
                    || typed_left.ty == self.session.types.borrow_mut().intern(Type::Error);

                if !is_valid {
                    self.error(
                        span,
                        DiagnosticCode::TypeMismatch,
                        &format!(
                            "Cannot coalesce type '{}' with '{}'.",
                            self.session.format_type(typed_left.ty),
                            self.session.format_type(typed_right.ty)
                        ),
                    );
                }

                (
                    TypedExprKind::NullCoalesce {
                        left: self.alloc(typed_left),
                        right: self.alloc(typed_right.clone()),
                    },
                    typed_right.ty,
                )
            }
            Type::Null => (
                TypedExprKind::NullCoalesce {
                    left: self.alloc(typed_left),
                    right: self.alloc(typed_right.clone()),
                },
                typed_right.ty,
            ),
            Type::Error | Type::Any => (
                TypedExprKind::NullCoalesce {
                    left: self.alloc(typed_left),
                    right: self.alloc(typed_right.clone()),
                },
                typed_right.ty,
            ),
            _ => {
                self.error(
                    span,
                    DiagnosticCode::TypeMismatch,
                    &format!(
                        "Left operand of '??' must be an optional type, found '{}'.",
                        self.session.format_type(typed_left.ty)
                    ),
                );
                (
                    TypedExprKind::NullCoalesce {
                        left: self.alloc(typed_left),
                        right: self.alloc(typed_right.clone()),
                    },
                    typed_right.ty,
                )
            }
        }
    }

    pub(crate) fn check_null_coalesce_assign_expr(
        &mut self,
        left: &Expr<'a>,
        right: &Expr<'a>,
        span: Span,
    ) -> (TypedExprKind<'a>, TypeId) {
        let typed_left = self.check_expr(left);
        let typed_right = self.check_expr(right);

        match self.get_type(typed_left.ty) {
            Type::Optional(inner) => {
                let expected = inner;
                if let TypedExprKind::Variable(ref left_name) = typed_left.kind
                    && !self.env.is_mutable(*left_name)
                {
                    self.error(
                        span,
                        DiagnosticCode::ImmutableAssignment,
                        &format!(
                            "Cannot mutate immutable variable '{}'.",
                            self.session.interner.borrow().lookup(*left_name)
                        ),
                    )
                }

                let is_valid = typed_right.ty == expected
                    || typed_right.ty == typed_left.ty
                    || typed_right.ty == self.session.types.borrow_mut().intern(Type::Error)
                    || typed_left.ty == self.session.types.borrow_mut().intern(Type::Error);

                if !is_valid {
                    self.error(
                        span,
                        DiagnosticCode::TypeMismatch,
                        &format!(
                            "Cannot assign type '{}' to variable of type '{}'.",
                            self.session.format_type(typed_right.ty),
                            self.session.format_type(typed_left.ty)
                        ),
                    );
                }

                (
                    TypedExprKind::NullCoalesceAssign {
                        left: self.alloc(typed_left.clone()),
                        right: self.alloc(typed_right),
                    },
                    typed_left.ty,
                )
            }
            Type::Error | Type::Any => (
                TypedExprKind::NullCoalesceAssign {
                    left: self.alloc(typed_left.clone()),
                    right: self.alloc(typed_right),
                },
                typed_left.ty,
            ),
            _ => {
                self.error(
                    span,
                    DiagnosticCode::TypeMismatch,
                    &format!(
                        "Left operand of '??=' must be an optional type, found '{}'.",
                        self.session.format_type(typed_left.ty)
                    ),
                );
                (
                    TypedExprKind::NullCoalesceAssign {
                        left: self.alloc(typed_left.clone()),
                        right: self.alloc(typed_right),
                    },
                    typed_left.ty,
                )
            }
        }
    }

    pub(crate) fn check_grouping_expr(
        &mut self,
        inner: &Expr<'a>,
        _span: Span,
    ) -> (TypedExprKind<'a>, TypeId) {
        let typed_inner = self.check_expr(inner);
        let ty = typed_inner.ty;
        (TypedExprKind::Grouping(self.alloc(typed_inner)), ty)
    }

    pub(crate) fn check_unary_expr(
        &mut self,
        op: &UnaryOp,
        right: &Expr<'a>,
        span: Span,
    ) -> (TypedExprKind<'a>, TypeId) {
        let typed_right = self.check_expr(right);
        if typed_right.ty == self.session.types.borrow_mut().intern(Type::Error) {
            return (
                TypedExprKind::Unary(op.clone(), self.alloc(typed_right)),
                self.session.types.borrow_mut().intern(Type::Error),
            );
        }

        let ty = match op {
            UnaryOp::Negate => {
                if typed_right.ty == self.session.types.borrow_mut().intern(Type::Int)
                    || typed_right.ty == self.session.types.borrow_mut().intern(Type::Float)
                {
                    typed_right.ty
                } else {
                    self.error(
                        span,
                        DiagnosticCode::TypeMismatch,
                        &format!(
                            "Cannot negate type '{}'.",
                            self.session.format_type(typed_right.ty)
                        ),
                    );
                    self.session.types.borrow_mut().intern(Type::Error)
                }
            }
            UnaryOp::Not => {
                if typed_right.ty == self.session.types.borrow_mut().intern(Type::Bool) {
                    typed_right.ty
                } else {
                    self.error(
                        span,
                        DiagnosticCode::TypeMismatch,
                        &format!(
                            "Cannot apply logical NOT to type '{}'. Expected Bool.",
                            self.session.format_type(typed_right.ty)
                        ),
                    );
                    self.session.types.borrow_mut().intern(Type::Error)
                }
            }
            UnaryOp::BitwiseNot => {
                if typed_right.ty == self.session.types.borrow_mut().intern(Type::Int) {
                    typed_right.ty
                } else {
                    self.error(
                        span,
                        DiagnosticCode::TypeMismatch,
                        &format!(
                            "Cannot apply bitwise NOT to type '{}'. Expected Int.",
                            self.session.format_type(typed_right.ty)
                        ),
                    );
                    self.session.types.borrow_mut().intern(Type::Error)
                }
            }
        };
        (
            TypedExprKind::Unary(op.clone(), self.alloc(typed_right)),
            ty,
        )
    }

    pub(crate) fn check_binary_expr(
        &mut self,
        left: &Expr<'a>,
        op: &BinaryOp,
        right: &Expr<'a>,
        span: Span,
    ) -> (TypedExprKind<'a>, TypeId) {
        let typed_left = self.check_expr(left);
        let typed_right = self.check_expr(right);

        if typed_left.ty == self.session.types.borrow_mut().intern(Type::Error)
            || typed_right.ty == self.session.types.borrow_mut().intern(Type::Error)
        {
            return (
                TypedExprKind::Binary(self.alloc(typed_left), op.clone(), self.alloc(typed_right)),
                self.session.types.borrow_mut().intern(Type::Error),
            );
        }

        let ty = match op {
            BinaryOp::Add
            | BinaryOp::Subtract
            | BinaryOp::Multiply
            | BinaryOp::Divide
            | BinaryOp::Modulo => {
                let int_ty = self.session.types.borrow_mut().intern(Type::Int);
                if (self.is_assignable(typed_left.ty, int_ty)
                    || typed_left.ty == self.session.types.borrow_mut().intern(Type::Float))
                    && self.is_assignable(typed_left.ty, typed_right.ty)
                {
                    typed_left.ty
                } else if *op == BinaryOp::Add
                    && typed_left.ty == self.session.types.borrow_mut().intern(Type::String)
                    && typed_right.ty == self.session.types.borrow_mut().intern(Type::String)
                {
                    self.session.types.borrow_mut().intern(Type::String)
                } else {
                    self.error(
                        span,
                        DiagnosticCode::TypeMismatch,
                        &format!(
                            "Cannot apply arithmetic operator to types '{}' and '{}'.",
                            self.session.format_type(typed_left.ty),
                            self.session.format_type(typed_right.ty)
                        ),
                    );
                    self.session.types.borrow_mut().intern(Type::Error)
                }
            }
            BinaryOp::BitwiseAnd
            | BinaryOp::BitwiseOr
            | BinaryOp::BitwiseXor
            | BinaryOp::ShiftLeft
            | BinaryOp::ShiftRight => {
                let int_ty = self.session.types.borrow_mut().intern(Type::Int);
                if self.is_assignable(typed_left.ty, int_ty)
                    && self.is_assignable(typed_right.ty, int_ty)
                {
                    int_ty
                } else {
                    self.error(
                        span,
                        DiagnosticCode::TypeMismatch,
                        &format!(
                            "Cannot apply bitwise operator to types '{}' and '{}'. Expected Int.",
                            self.session.format_type(typed_left.ty),
                            self.session.format_type(typed_right.ty)
                        ),
                    );
                    self.session.types.borrow_mut().intern(Type::Error)
                }
            }
            BinaryOp::Equal | BinaryOp::NotEqual => {
                if !self.is_assignable(typed_left.ty, typed_right.ty)
                    && !self.is_assignable(typed_right.ty, typed_left.ty)
                {
                    self.error(
                        span,
                        DiagnosticCode::TypeMismatch,
                        &format!(
                            "Cannot compare types '{}' and '{}' for equality.",
                            self.session.format_type(typed_left.ty),
                            self.session.format_type(typed_right.ty)
                        ),
                    );
                    self.session.types.borrow_mut().intern(Type::Error)
                } else {
                    self.session.types.borrow_mut().intern(Type::Bool)
                }
            }
            BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual => {
                if !self.is_assignable(typed_left.ty, typed_right.ty)
                    && !self.is_assignable(typed_right.ty, typed_left.ty)
                {
                    self.error(
                        span,
                        DiagnosticCode::TypeMismatch,
                        &format!(
                            "Cannot apply comparison to types '{}' and '{}'.",
                            self.session.format_type(typed_left.ty),
                            self.session.format_type(typed_right.ty)
                        ),
                    );
                }
                self.session.types.borrow_mut().intern(Type::Bool)
            }
        };
        (
            TypedExprKind::Binary(self.alloc(typed_left), op.clone(), self.alloc(typed_right)),
            ty,
        )
    }

    pub(crate) fn check_range_expr(
        &mut self,
        start: &Expr<'a>,
        end: &Expr<'a>,
        span: Span,
    ) -> (TypedExprKind<'a>, TypeId) {
        let int_ty = self.session.types.borrow_mut().intern(Type::Int);
        let typed_start = self.check_expr_with_expected(start, Some(int_ty));
        let typed_end = self.check_expr_with_expected(end, Some(int_ty));

        if typed_start.ty != self.session.types.borrow_mut().intern(Type::Int)
            || typed_end.ty != self.session.types.borrow_mut().intern(Type::Int)
        {
            self.error(
                span,
                DiagnosticCode::TypeMismatch,
                "Range bounds must be integers.",
            );
        }

        (
            TypedExprKind::Range {
                start: self.alloc(typed_start),
                end: self.alloc(typed_end),
            },
            self.session.types.borrow_mut().intern(Type::Range),
        )
    }

    pub(crate) fn check_logical_expr(
        &mut self,
        left: &Expr<'a>,
        op: &ast::LogicalOp,
        right: &Expr<'a>,
        span: Span,
    ) -> (TypedExprKind<'a>, TypeId) {
        let bool_ty = self.session.types.borrow_mut().intern(Type::Bool);
        let typed_left = self.check_expr_with_expected(left, Some(bool_ty));
        let typed_right = self.check_expr_with_expected(right, Some(bool_ty));

        if typed_left.ty != bool_ty || typed_right.ty != bool_ty {
            self.error(
                span,
                DiagnosticCode::TypeMismatch,
                "Logical operators require boolean operands.",
            );
        }

        (
            TypedExprKind::Logical(self.alloc(typed_left), op.clone(), self.alloc(typed_right)),
            bool_ty,
        )
    }
}
