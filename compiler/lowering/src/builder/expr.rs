pub mod access;
pub mod call;
pub mod collections;
pub mod control_flow;
pub mod literals;
pub mod operators;

use super::*;
use ast::{TypedExpr, TypedExprKind};
use mir::Value;

impl<'a> MirBuilder<'a> {
    pub(crate) fn lower_expr(&mut self, expr: &TypedExpr) -> Value {
        match &expr.kind {
            TypedExprKind::Integer(i) => self.lower_integer_expr(*i),
            TypedExprKind::Float(f) => self.lower_float_expr(*f),
            TypedExprKind::String(s) => self.lower_string_expr(*s),
            TypedExprKind::InterpolatedString(pieces) => {
                self.lower_interpolated_string_expr(pieces)
            }
            TypedExprKind::Boolean(b) => self.lower_boolean_expr(*b),
            TypedExprKind::Null => self.lower_null_expr(),
            TypedExprKind::Variable(name) => self.lower_variable_expr(*name),
            TypedExprKind::Array(elements) => self.lower_array_expr(elements),
            TypedExprKind::ArrayRepeat { value, count } => {
                self.lower_array_repeat_expr(value, count)
            }
            TypedExprKind::ListComprehension {
                expr: mapped_expr,
                item_name,
                iterator,
            } => self.lower_list_comprehension_expr(mapped_expr, *item_name, iterator),
            TypedExprKind::Match { value, arms } => self.lower_match_expr(value, arms),
            TypedExprKind::EnumVariant {
                enum_name,
                variant_name,
            } => self.lower_enum_variant_expr(*enum_name, *variant_name),
            TypedExprKind::IndexGet { object, index } => self.lower_index_get_expr(object, index),
            TypedExprKind::IndexSet {
                object,
                index,
                value,
            } => self.lower_index_set_expr(object, index, value),
            TypedExprKind::Grouping(inner) => self.lower_grouping_expr(inner),
            TypedExprKind::Get { object, name, is_static } => self.lower_get_expr(object, *name, *is_static),
            TypedExprKind::PostfixTry(inner) => self.lower_postfix_try_expr(inner),
            TypedExprKind::ForceUnwrap(inner) => self.lower_force_unwrap_expr(inner),
            TypedExprKind::OptionalGet { object, name } => {
                self.lower_optional_get_expr(object, *name)
            }
            TypedExprKind::NullCoalesce { left, right } => {
                self.lower_null_coalesce_expr(left, right)
            }
            TypedExprKind::Ternary {
                condition,
                true_expr,
                false_expr,
            } => self.lower_ternary_expr(condition, true_expr, false_expr),
            TypedExprKind::NullCoalesceAssign { left, right } => {
                self.lower_null_coalesce_assign_expr(left, right)
            }
            TypedExprKind::Set {
                object,
                name,
                value,
                is_static,
            } => self.lower_set_expr(object, *name, value, *is_static),
            TypedExprKind::Assign { name, value } => self.lower_assign_expr(*name, value),
            TypedExprKind::SelfRef => self.lower_self_ref_expr(),
            TypedExprKind::Call {
                callee,
                type_args,
                arguments,
            } => self.lower_call_expr(callee, type_args, arguments),
            TypedExprKind::Unary(op, right) => self.lower_unary_expr(op, right),
            TypedExprKind::Binary(left, op, right) => self.lower_binary_expr(left, op, right),
            TypedExprKind::Logical(left, op, right) => self.lower_logical_expr(left, op, right),
            TypedExprKind::Range { .. } => self.lower_range_expr(),
            TypedExprKind::Await(inner) => {
                let inner_val = self.lower_expr(inner);
                let tmp = self.new_temp();
                self.emit_assignment(tmp.clone(), expr.ty, mir::RValue::Await(inner_val));
                Value::Place(tmp)
            }
            TypedExprKind::Spawn(inner) => {
                let inner_val = self.lower_expr(inner);
                let tmp = self.new_temp();
                self.emit_assignment(tmp.clone(), expr.ty, mir::RValue::Spawn(inner_val));
                Value::Place(tmp)
            }
        }
    }
}
