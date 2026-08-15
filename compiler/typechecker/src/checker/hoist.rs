use super::*;
use session::types::Type;

impl<'a> TypeChecker<'a> {
    pub fn collect_declarations(&mut self, statements: &[Stmt<'a>]) {
        for stmt in statements {
            match &stmt.kind {
                StmtKind::Block(_stmts) => {
                    // Do not eagerly collect inside blocks here, wait until check_stmt enters the block
                }
                StmtKind::Class {
                    name,
                    type_params,
                    implements,
                    methods,
                    fields,
                    is_private: _,
                } => {
                    if !type_params.is_empty() {
                        self.generic_registry.register_class(*name, stmt.clone());
                        continue;
                    }

                    self.env.declare(
                        *name,
                        self.session
                            .types
                            .borrow_mut()
                            .intern(Type::Class(*name, type_params.clone())),
                    );
                    self.classes.insert(*name, std::collections::HashMap::new());

                    self.env.push_scope();
                    for tp in type_params {
                        self.env.declare(
                            *tp,
                            self.session.types.borrow_mut().intern(Type::Generic(*tp)),
                        );
                    }

                    let mut class_members = std::collections::HashMap::new();
                    let mut uninit_props = Vec::new();
                    let mut class_mutables_map = std::collections::HashMap::new();

                    for field in fields {
                        let (f_name, type_annotation, initializer, _is_weak, is_mutable) =
                            match &field.kind {
                                StmtKind::Var {
                                    name,
                                    type_annotation,
                                    initializer,
                                    is_weak,
                                    is_private: _,
                                } => (name, type_annotation, initializer, *is_weak, true),
                                StmtKind::Let {
                                    name,
                                    type_annotation,
                                    initializer,
                                    is_private: _,
                                } => (name, type_annotation, initializer, false, false),
                                _ => continue,
                            };

                        if initializer.is_none() {
                            uninit_props.push(*f_name);
                        }

                        let ty = if let Some(ann) = type_annotation {
                            
                            self.parse_type(ann, field.span)
                        } else if let Some(_init) = initializer {
                            self.session.types.borrow_mut().intern(Type::Any)
                        } else {
                            self.session.types.borrow_mut().intern(Type::Any)
                        };
                        class_members.insert(*f_name, ty);
                        class_mutables_map.insert(*f_name, is_mutable);
                    }

                    self.uninitialized_class_properties
                        .insert(*name, uninit_props);

                    for method in methods {
                        if let StmtKind::Func {
                            name: m_name,
                            params,
                            return_type,
                            ..
                        } = &method.kind
                        {
                            let ret_ty = if let Some(rt) = return_type {
                                self.parse_type(rt, method.span)
                            } else {
                                self.session.types.borrow_mut().intern(Type::Void)
                            };
                            let mut param_types = Vec::new();
                            for (_, pt) in params {
                                param_types.push(self.parse_type(pt, method.span));
                            }
                            class_members.insert(
                                *m_name,
                                self.session.types.borrow_mut().intern(Type::Function(
                                    Vec::new(),
                                    param_types,
                                    ret_ty,
                                )),
                            );
                        }
                    }

                    self.classes.insert(*name, class_members.clone());
                    self.class_mutables.insert(*name, class_mutables_map);
                    self.class_implements.insert(*name, implements.clone());
                    self.env.pop_scope();
                }
                StmtKind::Struct {
                    name,
                    type_params,
                    methods,
                    fields,
                    is_private: _,
                } => {
                    if !type_params.is_empty() {
                        self.generic_registry.register_class(*name, stmt.clone());
                        continue;
                    }

                    self.env.declare(
                        *name,
                        self.session
                            .types
                            .borrow_mut()
                            .intern(Type::Struct(*name, type_params.clone())),
                    );
                    self.classes.insert(*name, std::collections::HashMap::new());

                    self.env.push_scope();
                    for tp in type_params {
                        self.env.declare(
                            *tp,
                            self.session.types.borrow_mut().intern(Type::Generic(*tp)),
                        );
                    }

                    let mut struct_members = std::collections::HashMap::new();
                    let mut uninit_props = Vec::new();
                    let mut struct_mutables_map = std::collections::HashMap::new();

                    for field in fields {
                        let (f_name, type_annotation, initializer, is_mutable) = match &field.kind {
                            StmtKind::Var {
                                name,
                                type_annotation,
                                initializer,
                                ..
                            } => (name, type_annotation, initializer, true),
                            StmtKind::Let {
                                name,
                                type_annotation,
                                initializer,
                                ..
                            } => (name, type_annotation, initializer, false),
                            _ => continue,
                        };

                        if initializer.is_none() {
                            uninit_props.push(*f_name);
                        }

                        let ty = if let Some(ann) = type_annotation {
                            self.parse_type(ann, field.span)
                        } else if let Some(_init) = initializer {
                            self.session.types.borrow_mut().intern(Type::Any)
                        } else {
                            self.session.types.borrow_mut().intern(Type::Any)
                        };
                        struct_members.insert(*f_name, ty);
                        struct_mutables_map.insert(*f_name, is_mutable);
                    }

                    self.uninitialized_class_properties
                        .insert(*name, uninit_props);

                    for method in methods {
                        if let StmtKind::Func {
                            name: m_name,
                            params,
                            return_type,
                            ..
                        } = &method.kind
                        {
                            let ret_ty = if let Some(rt) = return_type {
                                self.parse_type(rt, method.span)
                            } else {
                                self.session.types.borrow_mut().intern(Type::Void)
                            };
                            let mut param_types = Vec::new();
                            for (_, pt) in params {
                                param_types.push(self.parse_type(pt, method.span));
                            }
                            struct_members.insert(
                                *m_name,
                                self.session.types.borrow_mut().intern(Type::Function(
                                    Vec::new(),
                                    param_types,
                                    ret_ty,
                                )),
                            );
                        }
                    }

                    self.classes.insert(*name, struct_members);
                    self.class_mutables.insert(*name, struct_mutables_map);
                    self.class_implements.insert(*name, Vec::new());
                    self.env.pop_scope();
                }
                StmtKind::Interface {
                    name,
                    methods,
                    is_private: _,
                } => {
                    let mut interface_methods = std::collections::HashMap::new();
                    for method in methods {
                        if let StmtKind::Func {
                            name: m_name,
                            params,
                            return_type,
                            ..
                        } = &method.kind
                        {
                            let ret_ty = if let Some(rt) = return_type {
                                self.parse_type(rt, method.span)
                            } else {
                                self.session.types.borrow_mut().intern(Type::Void)
                            };
                            let mut param_types = Vec::new();
                            for (_, pt) in params {
                                param_types.push(self.parse_type(pt, method.span));
                            }
                            interface_methods.insert(
                                *m_name,
                                self.session.types.borrow_mut().intern(Type::Function(
                                    Vec::new(),
                                    param_types,
                                    ret_ty,
                                )),
                            );
                        }
                    }
                    self.interfaces.insert(*name, interface_methods);
                    self.env.declare(
                        *name,
                        self.session
                            .types
                            .borrow_mut()
                            .intern(Type::Interface(*name)),
                    );
                }
                StmtKind::Enum {
                    name,
                    type_params,
                    variants,
                    is_private: _,
                } => {
                    self.env.declare(
                        *name,
                        self.session
                            .types
                            .borrow_mut()
                            .intern(Type::Enum(*name, type_params.clone())),
                    );
                    self.env.push_scope();
                    for tp in type_params {
                        self.env.declare(
                            *tp,
                            self.session.types.borrow_mut().intern(Type::Generic(*tp)),
                        );
                    }
                    let mut enum_variants = std::collections::HashMap::new();
                    for variant in variants {
                        let mut param_types = Vec::new();
                        if let Some(fields) = &variant.fields {
                            for field in fields {
                                param_types.push(self.parse_type(&field.ty, stmt.span));
                            }
                        }

                        let ret_ty = if type_params.is_empty() {
                            self.session
                                .types
                                .borrow_mut()
                                .intern(Type::Instance(*name))
                        } else {
                            let mut ret_args = Vec::new();
                            for tp in type_params {
                                ret_args.push(
                                    self.session.types.borrow_mut().intern(Type::Generic(*tp)),
                                );
                            }
                            self.session
                                .types
                                .borrow_mut()
                                .intern(Type::GenericInstance(*name, ret_args))
                        };
                        let variant_ty =
                            self.session
                                .types
                                .borrow_mut()
                                .intern(Type::EnumVariantConstructor(
                                    *name,
                                    variant.name,
                                    type_params.clone(),
                                    param_types,
                                    ret_ty,
                                ));
                        enum_variants.insert(variant.name, variant_ty);
                    }
                    self.env.pop_scope();
                    for (variant_name, variant_ty) in &enum_variants {
                        self.env.declare(*variant_name, *variant_ty);
                    }
                    self.enums.insert(*name, enum_variants);
                }
                StmtKind::ForeignFunc {
                    name,
                    type_params,
                    params,
                    return_type,
                    is_private: _,
                } => {
                    self.env.push_scope();
                    for tp in type_params {
                        self.env.declare(
                            *tp,
                            self.session.types.borrow_mut().intern(Type::Generic(*tp)),
                        );
                    }
                    let ret_ty = if let Some(rt) = return_type {
                        self.parse_type(rt, stmt.span)
                    } else {
                        self.session.types.borrow_mut().intern(Type::Void)
                    };
                    let mut param_types = Vec::new();
                    for (_, param_type_str) in params {
                        param_types.push(self.parse_type(param_type_str, stmt.span));
                    }
                    self.env.pop_scope();
                    self.env.declare(
                        *name,
                        self.session.types.borrow_mut().intern(Type::Function(
                            type_params.clone(),
                            param_types.clone(),
                            ret_ty,
                        )),
                    );
                }
                StmtKind::Func {
                    name,
                    type_params,
                    params,
                    return_type,
                    body: _,
                    is_private: _,
                } => {
                    if !type_params.is_empty() {
                        self.generic_registry.register_function(*name, stmt.clone());
                        continue;
                    }
                    let ret_ty = if let Some(rt) = return_type {
                        self.parse_type(rt, stmt.span)
                    } else {
                        self.session.types.borrow_mut().intern(Type::Void)
                    };
                    let mut param_types = Vec::new();
                    for (_, param_type_str) in params {
                        param_types.push(self.parse_type(param_type_str, stmt.span));
                    }

                    let func_ty = self.session.types.borrow_mut().intern(Type::Function(
                        type_params.clone(),
                        param_types.clone(),
                        ret_ty,
                    ));
                    if let Some(existing) = self.env.resolve(*name) {
                        if matches!(
                            self.get_type(existing),
                            Type::Function(..) | Type::OverloadedFunction(..)
                        ) {
                            let mut funcs = match self.get_type(existing) {
                                Type::OverloadedFunction(fs) => fs,
                                Type::Function(..) => vec![(*name, existing)],
                                _ => unreachable!(),
                            };
                            let mut mangled =
                                format!("_PO_{}", self.session.interner.borrow().lookup(*name));
                            for ty in &param_types {
                                mangled.push_str(
                                    &format!("_{}", self.session.format_type(*ty))
                                        .replace("<", "_")
                                        .replace(">", "")
                                        .replace(" ", "")
                                        .replace("?", "Opt")
                                        .replace("[]", "Arr"),
                                );
                            }
                            let mangled_sym = self.session.interner.borrow_mut().intern(&mangled);
                            funcs.push((mangled_sym, func_ty));
                            self.env.declare(
                                *name,
                                self.session
                                    .types
                                    .borrow_mut()
                                    .intern(Type::OverloadedFunction(funcs)),
                            );
                            self.env.declare(mangled_sym, func_ty);
                        } else {
                            self.env.declare(*name, func_ty);
                        }
                    } else {
                        self.env.declare(*name, func_ty);
                    }
                }
                _ => {}
            }
        }
    }
}
