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

        if let TypedExprKind::Get { object, name, is_static } = &callee.kind {
            let obj_val = self.lower_expr(object);
            let temp = self.new_temp();

            match self.session.types.borrow().get(object.ty) {
                Type::Interface(_, _) | Type::Any => {
                    // Dynamic dispatch
                    let __inst = Inst::Assign(
                        temp.clone(),
                        RValue::MethodCall(
                            obj_val.clone(),
                            self.session.interner.borrow().lookup(*name).to_string(),
                            arg_values,
                        ),
                    );
                    self.current().instructions.push(__inst);
                    return Value::Place(temp);
                }
                _ => {
                    // Static dispatch for Instances, Enums, Primitives (extensions)
                    let target_name = match self.session.types.borrow().get(object.ty) {
                        Type::Enum(n, _)
                        | Type::Instance(n)
                        | Type::Struct(n, _)
                        | Type::Class(n, _) => {
                            self.session.interner.borrow().lookup(*n).to_string()
                        }
                        _ => {
                            let target_name_raw = self.session.format_type(object.ty);
                            target_name_raw
                                .replace("<", "_")
                                .replace(">", "")
                                .replace(" ", "")
                                .replace(",", "_")
                        }
                    };
                    let is_actor = self.actor_names.contains(&target_name);

                    let actual_name = format!(
                        "{}::{}",
                        target_name,
                        self.session.interner.borrow().lookup(*name)
                    );
                    
                    let obj_val_for_push = obj_val.clone();
                    
                    if !*is_static {
                        arg_values.insert(0, obj_val);
                    }
                    
                    let __inst = if is_actor && self.session.interner.borrow().lookup(*name) != "init" {
                        Inst::Assign(temp.clone(), RValue::ActorMailboxPush(obj_val_for_push, actual_name, arg_values))
                    } else {
                        Inst::Assign(temp.clone(), RValue::Call(actual_name, arg_values))
                    };
                    self.current().instructions.push(__inst);
                    
                    return Value::Place(temp);
                }
            }
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

        let mut actual_name = self.session.interner.borrow().lookup(func_name).to_string();
        if actual_name == "hash" || actual_name == "equals" {
            let type_name = self.session.format_type(arguments[0].ty);
            actual_name = format!("{}_{}", actual_name, type_name);
        }

        let temp = self.new_temp();
        {
            let __inst = Inst::Assign(
                temp.clone(),
                RValue::Call(
                    actual_name,
                    arg_values,
                ),
            );
            self.current().instructions.push(__inst)
        };
        Value::Place(temp)
    }
}
