use super::*;
use ast::EnumVariant;
use session::types::Type;

impl<'a> TypeChecker<'a> {
    pub(crate) fn hoist_interface(&mut self, stmt: &Stmt<'a>, name: &session::Symbol, type_params: &[session::Symbol], methods: &[Stmt<'a>]) {
        if !type_params.is_empty() {
            self.generic_registry
                .register_interface(*name, stmt.clone());
            return;
        }

        let mut interface_methods = std::collections::HashMap::new();
        for method in methods {
            if let StmtKind::Func {
                name: m_name,
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
                .intern(Type::Interface(*name, type_params.to_vec())),
        );
    }

    pub(crate) fn hoist_enum(&mut self, stmt: &Stmt<'a>, name: &session::Symbol, type_params: &[session::Symbol], variants: &[EnumVariant<'a>], methods: &[Stmt<'a>]) {
        if !type_params.is_empty() {
            self.generic_registry.register_class(*name, stmt.clone());
        }
        self.env.declare(
            *name,
            self.session
                .types
                .borrow_mut()
                .intern(Type::Enum(*name, type_params.to_vec())),
        );
        self.env.push_scope();
        for tp in type_params {
            self.env.declare(
                *tp,
                self.session.types.borrow_mut().intern(Type::Generic(*tp)),
            );
        }
        self.enums.insert(*name, std::collections::HashMap::new());
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
                        type_params.to_vec(),
                        param_types,
                        ret_ty,
                    ));
            enum_variants.insert(variant.name, variant_ty);
        }

        let mut enum_methods = std::collections::HashMap::new();
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
                    enum_methods.insert(*m_name, func_ty);
                }
            }
        }
        self.classes.insert(*name, enum_methods);

        self.env.pop_scope();
        for (variant_name, variant_ty) in &enum_variants {
            self.env.declare(*variant_name, *variant_ty);
        }
        self.enums.insert(*name, enum_variants);
    }

    pub(crate) fn hoist_type_alias(&mut self, stmt: &Stmt<'a>, name: &session::Symbol, type_params: &[session::Symbol], target_type: &TypeExpr<'a>) {
        if !type_params.is_empty() {
            self.env.push_scope();
            for tp in type_params {
                self.env.declare(
                    *tp,
                    self.session.types.borrow_mut().intern(Type::Generic(*tp)),
                );
            }
        }
        let target_id = self.parse_type(target_type, stmt.span);
        if !type_params.is_empty() {
            self.env.pop_scope();
        }
        self.env.declare(
            *name,
            self.session.types.borrow_mut().intern(Type::TypeAlias(
                *name,
                type_params.to_vec(),
                target_id,
            )),
        );
    }
}
