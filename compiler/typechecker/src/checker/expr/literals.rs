use super::super::*;
use session::types::{Type, TypeId};

impl<'a> TypeChecker<'a> {
    pub(crate) fn check_integer_expr(&mut self, v: i64) -> (TypedExprKind<'a>, TypeId) {
        (
            TypedExprKind::Integer(v),
            self.session.types.borrow_mut().intern(Type::Int),
        )
    }

    pub(crate) fn check_float_expr(&mut self, v: f64) -> (TypedExprKind<'a>, TypeId) {
        (
            TypedExprKind::Float(v),
            self.session.types.borrow_mut().intern(Type::Float),
        )
    }

    pub(crate) fn check_string_expr(
        &mut self,
        v: session::interner::Symbol,
    ) -> (TypedExprKind<'a>, TypeId) {
        (
            TypedExprKind::String(v),
            self.session.types.borrow_mut().intern(Type::String),
        )
    }

    pub(crate) fn check_interpolated_string_expr(
        &mut self,
        pieces: &[Expr<'a>],
    ) -> (TypedExprKind<'a>, TypeId) {
        let mut typed_pieces = Vec::new();
        for piece in pieces {
            let typed_piece = self.check_expr(piece);
            match self.get_type(typed_piece.ty) {
                Type::Int | Type::Float | Type::String | Type::Bool | Type::Error => {}
                _ => {
                    self.error(piece.span, DiagnosticCode::TypeMismatch, &format!("Cannot interpolate type '{}'. Only Int, Float, String, and Bool are supported.", self.session.format_type(typed_piece.ty)));
                }
            }
            typed_pieces.push(typed_piece);
        }
        (
            TypedExprKind::InterpolatedString(typed_pieces),
            self.session.types.borrow_mut().intern(Type::String),
        )
    }

    pub(crate) fn check_boolean_expr(&mut self, v: bool) -> (TypedExprKind<'a>, TypeId) {
        (
            TypedExprKind::Bool(v),
            self.session.types.borrow_mut().intern(Type::Bool),
        )
    }

    pub(crate) fn check_null_expr(&mut self) -> (TypedExprKind<'a>, TypeId) {
        (
            TypedExprKind::Null,
            self.session.types.borrow_mut().intern(Type::Null),
        )
    }
}
