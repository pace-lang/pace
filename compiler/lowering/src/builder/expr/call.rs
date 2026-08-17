use super::super::*;
use ast::{TypedExpr, TypedExprKind};
use mir::{Inst, RValue, Value};
use session::types::Type;

impl<'a> MirBuilder<'a> {
    pub(crate) fn lower_enum_variant_expr(
        &mut self,
        enum_name: session::Symbol,
        variant_name: session::Symbol,
    ) -> Value {
        let variant_idx = self
            .enums_map
            .get(self.session.interner.borrow().lookup(enum_name))
            .and_then(|variants| {
                variants
                    .iter()
                    .position(|v| v == self.session.interner.borrow().lookup(variant_name))
            })
            .unwrap_or(0);
        let temp = self.new_temp();
        {
            let __inst = Inst::Assign(
                temp.clone(),
                RValue::ConstructVariant(
                    self.session.interner.borrow().lookup(enum_name).to_string(),
                    variant_idx,
                    Vec::new(),
                ),
            );
            self.current().instructions.push(__inst)
        };
        Value::Place(temp)
    }

    pub(crate) fn lower_call_expr(&mut self, callee: &TypedExpr, arguments: &[TypedExpr]) -> Value {
        let mut arg_values = Vec::new();
        for arg in arguments {
            arg_values.push(self.lower_expr(arg));
        }

        if let TypedExprKind::Get { object, name } = &callee.kind {
            let obj_val = self.lower_expr(object);
            let temp = self.new_temp();

            if let Type::Instance(class_name) | Type::GenericInstance(class_name, _) | Type::Enum(class_name, _) =
                self.session.types.borrow().get(object.ty)
            {
                let actual_name = format!(
                    "{}::{}",
                    self.session.interner.borrow().lookup(*class_name),
                    self.session.interner.borrow().lookup(*name)
                );
                arg_values.insert(0, obj_val);
                {
                    let __inst = Inst::Assign(temp.clone(), RValue::Call(actual_name, arg_values));
                    self.current().instructions.push(__inst)
                };
                return Value::Place(temp);
            }

            {
                let __inst = Inst::Assign(
                    temp.clone(),
                    RValue::MethodCall(
                        obj_val.clone(),
                        self.session.interner.borrow().lookup(*name).to_string(),
                        arg_values,
                    ),
                );
                self.current().instructions.push(__inst)
            };
            return Value::Place(temp);
        }

        if let TypedExprKind::EnumVariant {
            enum_name,
            variant_name,
        } = &callee.kind
        {
            let temp = self.new_temp();
            let variant_idx = self
                .enums_map
                .get(self.session.interner.borrow().lookup(*enum_name))
                .and_then(|variants| {
                    variants
                        .iter()
                        .position(|v| v == self.session.interner.borrow().lookup(*variant_name))
                })
                .unwrap_or(0);
            {
                let __inst = Inst::Assign(
                    temp.clone(),
                    RValue::ConstructVariant(
                        self.session
                            .interner
                            .borrow()
                            .lookup(*enum_name)
                            .to_string(),
                        variant_idx,
                        arg_values,
                    ),
                );
                self.current().instructions.push(__inst)
            };
            return Value::Place(temp);
        }

        if let Type::Struct(struct_name, _) = self.session.types.borrow().get(callee.ty).clone() {
            let obj_temp = self.new_temp();
            {
                let __inst = Inst::Assign(
                    obj_temp.clone(),
                    RValue::AllocateStruct(
                        self.session
                            .interner
                            .borrow()
                            .lookup(struct_name)
                            .to_string(),
                    ),
                );
                self.current().instructions.push(__inst)
            };

            let actual_name = format!(
                "{}::init",
                self.session.interner.borrow().lookup(struct_name)
            );
            arg_values.insert(0, Value::Place(obj_temp.clone()));
            let init_temp = self.new_temp();
            {
                let __inst = Inst::Assign(init_temp, RValue::Call(actual_name, arg_values));
                self.current().instructions.push(__inst)
            };

            return Value::Place(obj_temp);
        }

        if let Type::Class(class_name, _) = self.session.types.borrow().get(callee.ty).clone() {
            let obj_temp = self.new_temp();
            {
                let __inst = Inst::Assign(
                    obj_temp.clone(),
                    RValue::AllocateObject(
                        self.session
                            .interner
                            .borrow()
                            .lookup(class_name)
                            .to_string(),
                    ),
                );
                self.current().instructions.push(__inst)
            };

            let actual_name = format!(
                "{}::init",
                self.session.interner.borrow().lookup(class_name)
            );
            arg_values.insert(0, Value::Place(obj_temp.clone()));
            let init_temp = self.new_temp();
            {
                let __inst = Inst::Assign(init_temp, RValue::Call(actual_name, arg_values));
                self.current().instructions.push(__inst)
            };

            return Value::Place(obj_temp);
        }

        let mut func_name = if let TypedExprKind::Variable(name) = &callee.kind {
            *name
        } else {
            panic!("Only direct function calls by name are currently supported.");
        };

        if self.session.interner.borrow().lookup(func_name) == "print" && arguments.len() == 1 {
            match self.session.types.borrow().get(arguments[0].ty) {
                session::types::Type::String => {
                    func_name = self.session.interner.borrow_mut().intern("printStr")
                }
                session::types::Type::Float => {
                    func_name = self.session.interner.borrow_mut().intern("printFloat")
                }
                session::types::Type::Boolean => {
                    func_name = self.session.interner.borrow_mut().intern("printBool")
                }
                session::types::Type::Enum(_, _) | session::types::Type::Instance(_) => {
                    func_name = self.session.interner.borrow_mut().intern("printEnum")
                }
                _ => func_name = self.session.interner.borrow_mut().intern("printInt"),
            }
        }

        let temp = self.new_temp();
        {
            let __inst = Inst::Assign(
                temp.clone(),
                RValue::Call(
                    self.session.interner.borrow().lookup(func_name).to_string(),
                    arg_values,
                ),
            );
            self.current().instructions.push(__inst)
        };
        Value::Place(temp)
    }
}
