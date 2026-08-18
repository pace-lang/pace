mod access;
mod call;
mod collections;
mod control_flow;
mod literals;
mod operators;

use super::*;

impl<'a> TypeChecker<'a> {
    pub(crate) fn check_expr(&mut self, expr: &Expr<'a>) -> TypedExpr<'a> {
        self.check_expr_with_expected(expr, None)
    }

    pub(crate) fn check_expr_with_expected(
        &mut self,
        expr: &Expr<'a>,
        expected_ty: Option<TypeId>,
    ) -> TypedExpr<'a> {
        let (kind, ty) = match &expr.kind {
            ExprKind::Integer(v) => self.check_integer_expr(*v),
            ExprKind::Float(v) => self.check_float_expr(*v),
            ExprKind::String(v) => self.check_string_expr(*v),
            ExprKind::InterpolatedString(pieces) => self.check_interpolated_string_expr(pieces),
            ExprKind::Boolean(v) => self.check_boolean_expr(*v),
            ExprKind::Null => self.check_null_expr(),
            ExprKind::Variable(name) => self.check_variable_expr(*name, expr.span),
            ExprKind::Assign { name, value } => self.check_assign_expr(*name, value, expr.span),
            ExprKind::SelfRef => self.check_self_ref_expr(expr.span),
            ExprKind::ForceUnwrap(inner) => self.check_force_unwrap_expr(inner, expr.span),
            ExprKind::PostfixTry(inner) => self.check_postfix_try_expr(inner, expr.span),
            ExprKind::OptionalGet { object, name } => {
                self.check_optional_get_expr(object, *name, expr.span)
            }
            ExprKind::NullCoalesce { left, right } => {
                self.check_null_coalesce_expr(left, right, expr.span)
            }
            ExprKind::NullCoalesceAssign { left, right } => {
                self.check_null_coalesce_assign_expr(left, right, expr.span)
            }
            ExprKind::Ternary {
                condition,
                true_expr,
                false_expr,
            } => self.check_ternary_expr(condition, true_expr, false_expr, expr.span),
            ExprKind::Array(elements) => self.check_array_expr(elements, expr.span),
            ExprKind::ArrayRepeat { value, count } => {
                self.check_array_repeat_expr(value, count, expr.span)
            }
            ExprKind::ListComprehension {
                expr: mapped_expr,
                item_name,
                iterator,
            } => self.check_list_comprehension_expr(mapped_expr, item_name, iterator, expr.span),
            ExprKind::IndexGet { object, index } => {
                self.check_index_get_expr(object, index, expr.span)
            }
            ExprKind::IndexSet {
                object,
                index,
                value,
            } => self.check_index_set_expr(object, index, value, expr.span),
            ExprKind::Get { object, name } => self.check_get_expr(object, *name, expr.span),
            ExprKind::Set {
                object,
                name,
                value,
            } => self.check_set_expr(object, *name, value, expr.span),
            ExprKind::Grouping(inner) => self.check_grouping_expr(inner, expr.span),
            ExprKind::Match { value, arms } => self.check_match_expr(value, arms, expr.span),
            ExprKind::Call {
                callee,
                type_args,
                arguments,
            } => self.check_call_expr(callee, type_args, arguments, expected_ty, expr.span),
            ExprKind::Unary(op, right) => self.check_unary_expr(op, right, expr.span),
            ExprKind::Range { start, end } => self.check_range_expr(start, end, expr.span),
            ExprKind::Binary(left, op, right) => self.check_binary_expr(left, op, right, expr.span),
        };
        TypedExpr::new(kind, ty, expr.span)
    }
}
