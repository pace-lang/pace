use super::super::*;
use ast::{TypedExpr, TypedExprKind};
use mir::{Inst, Place, RValue, Terminator, Value};

impl<'a> MirBuilder<'a> {
    pub(crate) fn lower_array_expr(&mut self, elements: &[TypedExpr]) -> Value {
        let mut vals = Vec::new();
        for el in elements {
            vals.push(self.lower_expr(el));
        }
        let temp = self.new_temp();
        {
            let __inst = Inst::Assign(temp.clone(), RValue::Array(vals, false));
            self.current().instructions.push(__inst)
        };
        Value::Place(temp)
    }

    pub(crate) fn lower_array_repeat_expr(
        &mut self,
        value: &TypedExpr,
        count: &TypedExpr,
    ) -> Value {
        let val = self.lower_expr(value);
        let count_val = self.lower_expr(count);
        let temp = self.new_temp();
        {
            let __inst = Inst::Assign(temp.clone(), RValue::ArrayRepeat(val, count_val, false));
            self.current().instructions.push(__inst)
        };
        Value::Place(temp)
    }


}
