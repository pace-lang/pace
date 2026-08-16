use super::super::*;
use ast::TypedExpr;
use mir::{Value, Place, RValue, Inst, Terminator};

impl<'a> MirBuilder<'a> {
    pub(crate) fn lower_variable_expr(&mut self, name: session::Symbol) -> Value {
        Value::Place(Place::Var(
            self.session.interner.borrow().lookup(name).to_string(),
        ))
    }

    pub(crate) fn lower_assign_expr(
        &mut self,
        name: session::Symbol,
        value: &TypedExpr,
    ) -> Value {
        let val = self.lower_expr(value);
        {
            let __inst = Inst::Assign(
                Place::Var(self.session.interner.borrow().lookup(name).to_string()),
                RValue::Use(val.clone()),
            );
            self.current().instructions.push(__inst)
        };
        val
    }

    pub(crate) fn lower_index_get_expr(
        &mut self,
        object: &TypedExpr,
        index: &TypedExpr,
    ) -> Value {
        let obj_val = self.lower_expr(object);
        let idx_val = self.lower_expr(index);
        let temp = self.new_temp();
        {
            let __inst = Inst::Assign(temp.clone(), RValue::IndexGet(obj_val, idx_val));
            self.current().instructions.push(__inst)
        };
        Value::Place(temp)
    }

    pub(crate) fn lower_index_set_expr(
        &mut self,
        object: &TypedExpr,
        index: &TypedExpr,
        value: &TypedExpr,
    ) -> Value {
        let obj_val = self.lower_expr(object);
        let idx_val = self.lower_expr(index);
        let val_val = self.lower_expr(value);
        {
            let __inst = Inst::IndexSet(obj_val, idx_val, val_val.clone());
            self.current().instructions.push(__inst)
        };
        val_val
    }

    pub(crate) fn lower_get_expr(
        &mut self,
        object: &TypedExpr,
        name: session::Symbol,
    ) -> Value {
        let obj_val = self.lower_expr(object);
        let temp = self.new_temp();
        {
            let __inst = Inst::Assign(
                temp.clone(),
                RValue::GetProperty(
                    obj_val,
                    self.session.interner.borrow().lookup(name).to_string(),
                ),
            );
            self.current().instructions.push(__inst)
        };
        Value::Place(temp)
    }

    pub(crate) fn lower_set_expr(
        &mut self,
        object: &TypedExpr,
        name: session::Symbol,
        value: &TypedExpr,
    ) -> Value {
        let obj_val = self.lower_expr(object);
        let val_val = self.lower_expr(value);
        {
            let __inst = Inst::SetProperty(
                obj_val,
                self.session.interner.borrow().lookup(name).to_string(),
                val_val.clone(),
            );
            self.current().instructions.push(__inst)
        };
        val_val
    }

    pub(crate) fn lower_self_ref_expr(&mut self) -> Value {
        Value::Place(Place::Var("self".to_string()))
    }

    pub(crate) fn lower_optional_get_expr(
        &mut self,
        object: &TypedExpr,
        name: session::Symbol,
    ) -> Value {
        let obj_val = self.lower_expr(object);
        let temp = self.new_temp();

        let then_block = self.new_block();
        let else_block = self.new_block();
        let merge_block = self.new_block();

        let is_null_temp = self.new_temp();
        {
            let __inst = Inst::Assign(
                is_null_temp.clone(),
                RValue::BinaryOp(ast::BinaryOp::Equal, obj_val.clone(), Value::Null),
            );
            self.current().instructions.push(__inst)
        };

        self.current().terminator = Some(Terminator::Branch {
            cond: Value::Place(is_null_temp),
            then_block,
            else_block,
        });

        self.current_block = then_block;
        {
            let __inst = Inst::Assign(temp.clone(), RValue::Use(Value::Null));
            self.current().instructions.push(__inst)
        };
        self.current().terminator = Some(Terminator::Jump(merge_block));

        self.current_block = else_block;
        {
            let __inst = Inst::Assign(
                temp.clone(),
                RValue::GetProperty(
                    obj_val,
                    self.session.interner.borrow().lookup(name).to_string(),
                ),
            );
            self.current().instructions.push(__inst)
        };
        self.current().terminator = Some(Terminator::Jump(merge_block));

        self.current_block = merge_block;
        Value::Place(temp)
    }
}
