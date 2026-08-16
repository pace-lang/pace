use super::super::*;
use session::types::{Type, TypeId};
use session::interner::Symbol;

impl<'a> TypeChecker<'a> {
    pub(crate) fn check_array_expr(&mut self, elements: &[Expr<'a>], span: Span) -> (TypedExprKind<'a>, TypeId) {
        if elements.is_empty() {
            self.error(
                span,
                DiagnosticCode::TypeMismatch,
                "Cannot infer type of empty array literal.",
            );
            (
                TypedExprKind::Array(Vec::new()),
                self.session.types.borrow_mut().intern(Type::Error),
            )
        } else {
            let mut typed_elements = Vec::new();
            let first_typed = self.check_expr(&elements[0]);
            let elem_type = first_typed.ty;
            typed_elements.push(first_typed);

            for elem in elements.iter().skip(1) {
                let next_typed = self.check_expr(elem);
                if next_typed.ty != elem_type
                    && next_typed.ty != self.session.types.borrow_mut().intern(Type::Error)
                    && elem_type != self.session.types.borrow_mut().intern(Type::Error)
                {
                    self.error(span, DiagnosticCode::TypeMismatch, &format!("Array elements have inconsistent types: expected '{}', found '{}'.", self.session.format_type(elem_type), self.session.format_type(next_typed.ty)));
                }
                typed_elements.push(next_typed);
            }
            (
                TypedExprKind::Array(typed_elements),
                self.session
                    .types
                    .borrow_mut()
                    .intern(Type::Array(elem_type)),
            )
        }
    }

    pub(crate) fn check_array_repeat_expr(&mut self, value: &Expr<'a>, count: &Expr<'a>, span: Span) -> (TypedExprKind<'a>, TypeId) {
        let typed_value = self.check_expr(value);
        let typed_count = self.check_expr(count);
        if typed_count.ty != self.session.types.borrow_mut().intern(Type::Int)
            && typed_count.ty != self.session.types.borrow_mut().intern(Type::Error)
        {
            self.error(
                span,
                DiagnosticCode::TypeMismatch,
                &format!(
                    "Array repeat count must be 'Int', found '{}'.",
                    self.session.format_type(typed_count.ty)
                ),
            );
        }
        let ty = self
            .session
            .types
            .borrow_mut()
            .intern(Type::Array(typed_value.ty));
        (
            TypedExprKind::ArrayRepeat {
                value: self.alloc(typed_value),
                count: self.alloc(typed_count),
            },
            ty,
        )
    }

    pub(crate) fn check_list_comprehension_expr(
        &mut self,
        mapped_expr: &Expr<'a>,
        item_name: &Symbol,
        iterator: &Expr<'a>,
        span: Span,
    ) -> (TypedExprKind<'a>, TypeId) {
        let typed_iterator = self.check_expr(iterator);

        let item_type = match self.get_type(typed_iterator.ty) {
            Type::Range => self.session.types.borrow_mut().intern(Type::Int),
            Type::Array(inner) => inner,
            Type::Error => self.session.types.borrow_mut().intern(Type::Error),
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
        let typed_expr = self.check_expr(mapped_expr);
        self.env.pop_scope();

        let ty = self
            .session
            .types
            .borrow_mut()
            .intern(Type::Array(typed_expr.ty));
        (
            TypedExprKind::ListComprehension {
                expr: self.alloc(typed_expr),
                item_name: *item_name,
                iterator: self.alloc(typed_iterator),
            },
            ty,
        )
    }
}
