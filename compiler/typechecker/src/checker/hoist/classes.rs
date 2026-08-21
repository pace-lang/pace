use super::*;
use session::types::Type;

impl<'a> TypeChecker<'a> {
    pub(crate) fn hoist_class(&mut self, stmt: &Stmt<'a>, name: &session::Symbol, type_params: &[session::Symbol], implements: &[TypeExpr<'a>], methods: &[Stmt<'a>], fields: &[Stmt<'a>], is_actor: bool) {
        if !type_params.is_empty() {
            self.generic_registry.register_class(*name, stmt.clone());
            return;
        }

        self.env.declare(
            *name,
            self.session
                .types
                .borrow_mut()
                .intern(Type::Class(*name, type_params.to_vec())),
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
            let (f_name, type_annotation, initializer, _is_weak, is_mutable, is_static) =
                match &field.kind {
                    StmtKind::Var {
                        name,
                        type_annotation,
                        initializer,
                        is_weak,
                        is_private: _,
                        is_static,
                    } => (name, type_annotation, initializer, *is_weak, true, *is_static),
                    StmtKind::Let {
                        name,
                        type_annotation,
                        initializer,
                        is_private: _,
                        is_static,
                    } => (name, type_annotation, initializer, false, false, *is_static),
                    _ => continue,
                };

            if initializer.is_none() && !is_static {
                uninit_props.push(*f_name);
            }

            let ty = if let Some(ann) = type_annotation {
                self.parse_type(ann, field.span)
            } else if let Some(_init) = initializer {
                self.session.types.borrow_mut().intern(Type::Any)
            } else {
                self.session.types.borrow_mut().intern(Type::Any)
            };
            if is_static {
                self.class_static_members.entry(*name).or_default().insert(*f_name, ty);
                self.class_static_mutables.entry(*name).or_default().insert(*f_name, is_mutable);
            } else {
                class_members.insert(*f_name, ty);
                class_mutables_map.insert(*f_name, is_mutable);
            }
        }

        if is_actor {
            class_members.insert(
                self.session.interner.borrow_mut().intern("__mailbox"),
                self.session.types.borrow_mut().intern(Type::Int)
            );
            class_mutables_map.insert(
                self.session.interner.borrow_mut().intern("__mailbox"),
                false
            );
        }

        self.uninitialized_class_properties
            .insert(*name, uninit_props);

        for method in methods {
            if let StmtKind::Func {
                name: m_name,
                params,
                return_type,
                is_async,
                is_static,
                ..
            } = &method.kind
            {
                let mut ret_ty = if let Some(rt) = return_type {
                    self.parse_type(rt, method.span)
                } else {
                    self.session.types.borrow_mut().intern(Type::Void)
                };
                if *is_async || is_actor {
                    ret_ty = self.session.types.borrow_mut().intern(Type::Task(ret_ty));
                }
                let mut param_types = Vec::new();
                for (_, pt) in params {
                    param_types.push(self.parse_type(pt, method.span));
                }
                let func_ty = self.session.types.borrow_mut().intern(Type::Function(
                    Vec::new(),
                    param_types,
                    ret_ty,
                ));
                
                if *is_static {
                    self.class_static_members.entry(*name).or_default().insert(*m_name, func_ty);
                } else {
                    class_members.insert(*m_name, func_ty);
                }
            }
        }

        self.classes.insert(*name, class_members.clone());
        self.class_mutables.insert(*name, class_mutables_map);
        let mut parsed_implements = Vec::new();
        for imp in implements {
            parsed_implements.push(self.parse_type(imp, stmt.span));
        }
        self.class_implements.insert(*name, parsed_implements);
        self.env.pop_scope();
    }

    pub(crate) fn hoist_struct(&mut self, stmt: &Stmt<'a>, name: &session::Symbol, type_params: &[session::Symbol], methods: &[Stmt<'a>], fields: &[Stmt<'a>]) {
        if !type_params.is_empty() {
            self.generic_registry.register_class(*name, stmt.clone());
            return;
        }

        self.env.declare(
            *name,
            self.session
                .types
                .borrow_mut()
                .intern(Type::Struct(*name, type_params.to_vec())),
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
            let (f_name, type_annotation, initializer, is_mutable, is_static) = match &field.kind {
                StmtKind::Var {
                    name,
                    type_annotation,
                    initializer,
                    is_static,
                    ..
                } => (name, type_annotation, initializer, true, *is_static),
                StmtKind::Let {
                    name,
                    type_annotation,
                    initializer,
                    is_static,
                    ..
                } => (name, type_annotation, initializer, false, *is_static),
                _ => continue,
            };

            if initializer.is_none() && !is_static {
                uninit_props.push(*f_name);
            }

            let ty = if let Some(ann) = type_annotation {
                self.parse_type(ann, field.span)
            } else if let Some(_init) = initializer {
                self.session.types.borrow_mut().intern(Type::Any)
            } else {
                self.session.types.borrow_mut().intern(Type::Any)
            };
            if is_static {
                self.class_static_members.entry(*name).or_default().insert(*f_name, ty);
                self.class_static_mutables.entry(*name).or_default().insert(*f_name, is_mutable);
            } else {
                struct_members.insert(*f_name, ty);
                struct_mutables_map.insert(*f_name, is_mutable);
            }
        }

        self.uninitialized_class_properties
            .insert(*name, uninit_props);

        for method in methods {
            if let StmtKind::Func {
                name: m_name,
                params,
                return_type,
                is_async,
                is_static,
                ..
            } = &method.kind
            {
                let mut ret_ty = if let Some(rt) = return_type {
                    self.parse_type(rt, method.span)
                } else {
                    self.session.types.borrow_mut().intern(Type::Void)
                };
                if *is_async {
                    ret_ty = self.session.types.borrow_mut().intern(Type::Task(ret_ty));
                }
                let mut param_types = Vec::new();
                for (_, pt) in params {
                    param_types.push(self.parse_type(pt, method.span));
                }
                let func_ty = self.session.types.borrow_mut().intern(Type::Function(
                    Vec::new(),
                    param_types,
                    ret_ty,
                ));
                if *is_static {
                    self.class_static_members.entry(*name).or_default().insert(*m_name, func_ty);
                } else {
                    struct_members.insert(*m_name, func_ty);
                }
            }
        }

        self.classes.insert(*name, struct_members);
        self.class_mutables.insert(*name, struct_mutables_map);
        self.class_implements.insert(*name, Vec::new());
        self.env.pop_scope();
    }

    pub(crate) fn hoist_extension(&mut self, stmt: &Stmt<'a>, target_type: &TypeExpr<'a>, type_params: &[session::Symbol], methods: &[Stmt<'a>]) {
        self.env.push_scope();
        for tp in type_params {
            self.env.declare(
                *tp,
                self.session.types.borrow_mut().intern(Type::Generic(*tp)),
            );
        }

        let target_sym = match target_type {
            TypeExpr::Named(sym) | TypeExpr::GenericInstance(sym, _) => *sym,
            TypeExpr::Array(_) => self.session.interner.borrow_mut().intern("$ArrayExtension"),
            _ => {
                let target_id = self.parse_type(target_type, stmt.span);
                let type_str = self.session.format_type(target_id);
                self.session.interner.borrow_mut().intern(&type_str)
            }
        };

        if !type_params.is_empty() {
            self.generic_registry
                .register_extension(target_sym, stmt.clone());
        }

        let _target_id = self.parse_type(target_type, stmt.span);

        let mut ext_methods = self.extensions.remove(&target_sym).unwrap_or_default();
        for method in methods {
            if let StmtKind::Func {
                name,
                type_params: method_type_params,
                params,
                return_type,
                is_async,
                ..
            } = &method.kind
            {
                let mut ret_ty = if let Some(rt) = return_type {
                    self.parse_type(rt, method.span)
                } else {
                    self.session.types.borrow_mut().intern(Type::Void)
                };
                if *is_async {
                    ret_ty = self.session.types.borrow_mut().intern(Type::Task(ret_ty));
                }
                let mut param_types = Vec::new();
                for (_, pt) in params {
                    param_types.push(self.parse_type(pt, method.span));
                }

                ext_methods.insert(
                    *name,
                    self.session.types.borrow_mut().intern(Type::Function(
                        method_type_params.clone(),
                        param_types,
                        ret_ty,
                    )),
                );
            }
        }

        self.extensions.insert(target_sym, ext_methods);
        self.env.pop_scope();
    }
}
