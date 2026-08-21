use super::*;
use session::types::Type;

impl<'a> TypeChecker<'a> {
    pub(crate) fn check_if_stmt(
        &mut self,
        condition: &'a Expr<'a>,
        then_branch: &'a Stmt<'a>,
        else_branch: &Option<&'a Stmt<'a>>,
    ) -> TypedStmtKind<'a> {
        let typed_condition = self.check_expr(condition);
        if typed_condition.ty != self.session.types.borrow_mut().intern(Type::Bool)
            && typed_condition.ty != self.session.types.borrow_mut().intern(Type::Error)
        {
            self.error(
                condition.span,
                DiagnosticCode::TypeMismatch,
                &format!(
                    "Expected 'Bool' for if condition, found '{}'.",
                    self.session.format_type(typed_condition.ty)
                ),
            )
        }

        let typed_then = self.check_stmt(then_branch);
        let typed_else = else_branch.as_ref().map(|e_branch| {
            let stmt = self.check_stmt(e_branch);
            self.alloc(stmt)
        });
        TypedStmtKind::If {
            condition: self.alloc(typed_condition),
            then_branch: self.alloc(typed_then),
            else_branch: typed_else,
        }
    }

    pub(crate) fn check_while_stmt(
        &mut self,
        condition: &'a Expr<'a>,
        body: &'a Stmt<'a>,
    ) -> TypedStmtKind<'a> {
        let typed_condition = self.check_expr(condition);
        if typed_condition.ty != self.session.types.borrow_mut().intern(Type::Bool)
            && typed_condition.ty != self.session.types.borrow_mut().intern(Type::Error)
        {
            self.error(
                condition.span,
                DiagnosticCode::TypeMismatch,
                &format!(
                    "Expected 'Bool' for while condition, found '{}'.",
                    self.session.format_type(typed_condition.ty)
                ),
            )
        }

        let typed_body = self.check_stmt(body);
        TypedStmtKind::While {
            condition: self.alloc(typed_condition),
            body: self.alloc(typed_body),
        }
    }

    pub(crate) fn check_for_stmt(
        &mut self,
        item_name: &session::Symbol,
        iterator: &'a Expr<'a>,
        body: &'a Stmt<'a>,
        span: Span,
    ) -> TypedStmtKind<'a> {
        let typed_iterator = self.check_expr(iterator);

        let item_type = match self.get_type(typed_iterator.ty) {
            Type::Range => self.session.types.borrow_mut().intern(Type::Int),
            Type::Array(inner) => inner,
            Type::Error => self.session.types.borrow_mut().intern(Type::Error),
            Type::Instance(class_name) => {
                let mut item_ty = None;
                if let Some(implements) = self.class_implements.get(&class_name) {
                    let iterable_sym = self.session.interner.borrow_mut().intern("Iterable");
                    for imp_ty in implements {
                        if let Type::GenericInstance(name, args) = self.get_type(*imp_ty)
                            && name == iterable_sym
                            && args.len() == 1
                        {
                            item_ty = Some(args[0]);
                            break;
                        }
                    }
                }
                if let Some(ty) = item_ty {
                    ty
                } else {
                    self.error(
                        span,
                        DiagnosticCode::TypeMismatch,
                        &format!(
                            "Cannot iterate over type '{}' because it does not implement 'Iterable<T>'.",
                            self.session.format_type(typed_iterator.ty)
                        ),
                    );
                    self.session.types.borrow_mut().intern(Type::Error)
                }
            }
            _ => {
                self.error(
                    span,
                    DiagnosticCode::TypeMismatch,
                    &format!(
                        "Cannot iterate over non-iterable type '{}'.",
                        self.session.format_type(typed_iterator.ty)
                    ),
                );
                self.session.types.borrow_mut().intern(Type::Error)
            }
        };

        self.env.push_scope();
        self.env.declare_var(*item_name, item_type, false);
        let typed_body = self.check_stmt(body);
        self.env.pop_scope();
        TypedStmtKind::For {
            item_name: *item_name,
            iterator: self.alloc(typed_iterator),
            body: self.alloc(typed_body),
            item_ty: item_type,
        }
    }

    pub(crate) fn check_return_stmt(
        &mut self,
        value: &Option<&'a Expr<'a>>,
        span: Span,
    ) -> TypedStmtKind<'a> {
        let typed_val = if let Some(val) = value {
            let mut expected_ret = self.current_return_type;

            if self.in_async_context {
                if let Some(expected_ty) = expected_ret {
                    if let Type::Task(inner_type_id) = self.get_type(expected_ty).clone() {
                        expected_ret = Some(inner_type_id);
                    }
                }
            }

            let mut tv = self.check_expr_with_expected(val, expected_ret);

            if let Some(expected_ty) = expected_ret {
                if !self.is_assignable(tv.ty, expected_ty)
                    && tv.ty != self.session.types.borrow_mut().intern(Type::Error)
                    && expected_ty != self.session.types.borrow_mut().intern(Type::Error)
                    && expected_ty != self.session.types.borrow_mut().intern(Type::Any)
                {
                    self.error(
                        val.span,
                        DiagnosticCode::TypeMismatch,
                        &format!(
                            "Expected return type '{}', found '{}'.",
                            self.session.format_type(expected_ty),
                            self.session.format_type(tv.ty)
                        ),
                    );
                    tv.ty = self.session.types.borrow_mut().intern(Type::Error);
                }
            } else {
                self.error(
                    span,
                    DiagnosticCode::TypeMismatch,
                    "Cannot return from outside a function.",
                );
            }
            Some(tv)
        } else {
            let mut expected_ret = self.current_return_type;
            if self.in_async_context {
                if let Some(expected_ty) = expected_ret {
                    if let Type::Task(inner_type_id) = self.get_type(expected_ty).clone() {
                        expected_ret = Some(inner_type_id);
                    }
                }
            }

            if expected_ret.is_some()
                && *expected_ret.as_ref().unwrap() != self.session.types.borrow_mut().intern(Type::Void)
            {
                self.error(
                    span,
                    DiagnosticCode::TypeMismatch,
                    &format!(
                        "Expected return type '{}', found 'Void'.",
                        self.session.format_type(*expected_ret.as_ref().unwrap())
                    ),
                );
            }
            None
        };
        TypedStmtKind::Return {
            value: typed_val.map(|e| self.alloc(e)),
        }
    }
}
