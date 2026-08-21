use super::super::*;
use ast::TypedExpr;
use mir::{Inst, RValue, Value};

impl<'a> MirBuilder<'a> {
    pub(crate) fn lower_integer_expr(&mut self, i: i64) -> Value {
        Value::Int(i)
    }

    pub(crate) fn lower_float_expr(&mut self, f: f64) -> Value {
        Value::Float(f)
    }

    pub(crate) fn lower_string_expr(&mut self, s: session::Symbol) -> Value {
        Value::String(self.session.interner.borrow().lookup(s).to_string())
    }

    pub(crate) fn lower_interpolated_string_expr(&mut self, pieces: &[TypedExpr]) -> Value {
        if pieces.is_empty() {
            return Value::String("".to_string());
        }

        let mut current_str_val = None;

        for piece in pieces {
            let mut piece_val = self.lower_expr(piece);

            match self.session.types.borrow().get(piece.ty) {
                session::types::Type::Int => {
                    let temp = self.new_temp();
                    {
                        let __inst = Inst::Assign(
                            temp.clone(),
                            RValue::Call("pace_int_to_string".to_string(), vec![piece_val]),
                        );
                        self.current().instructions.push(__inst)
                    };
                    piece_val = Value::Place(temp);
                }
                session::types::Type::Float => {
                    let temp = self.new_temp();
                    {
                        let __inst = Inst::Assign(
                            temp.clone(),
                            RValue::Call("pace_float_to_string".to_string(), vec![piece_val]),
                        );
                        self.current().instructions.push(__inst)
                    };
                    piece_val = Value::Place(temp);
                }
                session::types::Type::Bool => {
                    let temp = self.new_temp();
                    {
                        let __inst = Inst::Assign(
                            temp.clone(),
                            RValue::Call("pace_bool_to_string".to_string(), vec![piece_val]),
                        );
                        self.current().instructions.push(__inst)
                    };
                    piece_val = Value::Place(temp);
                }
                _ => {}
            }

            if let Some(current) = current_str_val {
                let temp = self.new_temp();
                {
                    let __inst = Inst::Assign(
                        temp.clone(),
                        RValue::Call("pace_string_concat".to_string(), vec![current, piece_val]),
                    );
                    self.current().instructions.push(__inst)
                };
                current_str_val = Some(Value::Place(temp));
            } else {
                current_str_val = Some(piece_val);
            }
        }

        current_str_val.unwrap()
    }

    pub(crate) fn lower_boolean_expr(&mut self, b: bool) -> Value {
        Value::Bool(b)
    }

    pub(crate) fn lower_null_expr(&mut self) -> Value {
        Value::Null
    }
}
