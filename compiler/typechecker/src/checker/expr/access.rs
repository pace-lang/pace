use super::super::*;
use session::interner::Symbol;
use session::types::{Type, TypeId};

impl<'a> TypeChecker<'a> {
    pub(crate) fn check_variable_expr(
        &mut self,
        name: Symbol,
        span: Span,
    ) -> (TypedExprKind<'a>, TypeId) {
        if let Some(ty) = self.env.resolve(name) {
            (TypedExprKind::Variable(name), ty)
        } else if let Some(generic_stmt) = self.generic_registry.get_class(name).cloned() {
            if let StmtKind::Class { type_params, .. } = &generic_stmt.kind {
                (
                    TypedExprKind::Variable(name),
                    self.session
                        .types
                        .borrow_mut()
                        .intern(Type::Class(name, type_params.clone())),
                )
            } else {
                unreachable!()
            }
        } else if let Some(generic_stmt) = self.generic_registry.get_function(name).cloned() {
            if let StmtKind::Func {
                type_params,
                params,
                return_type,
                ..
            } = &generic_stmt.kind
            {
                self.env.push_scope();
                for tp in type_params {
                    self.env.declare(
                        *tp,
                        self.session.types.borrow_mut().intern(Type::Generic(*tp)),
                    );
                }

                let mut param_types = Vec::new();
                for (_, ty) in params {
                    param_types.push(self.parse_type(ty, span));
                }
                let ret_ty = if let Some(ty) = return_type {
                    self.parse_type(ty, span)
                } else {
                    self.session.types.borrow_mut().intern(Type::Void)
                };

                self.env.pop_scope();

                (
                    TypedExprKind::Variable(name),
                    self.session.types.borrow_mut().intern(Type::Function(
                        type_params.clone(),
                        param_types,
                        ret_ty,
                    )),
                )
            } else if let StmtKind::ForeignFunc {
                type_params,
                params,
                return_type,
                ..
            } = &generic_stmt.kind
            {
                self.env.push_scope();
                for tp in type_params {
                    self.env.declare(
                        *tp,
                        self.session.types.borrow_mut().intern(Type::Generic(*tp)),
                    );
                }

                let mut param_types = Vec::new();
                for (_, ty) in params {
                    param_types.push(self.parse_type(ty, span));
                }
                let ret_ty = if let Some(ty) = return_type {
                    self.parse_type(ty, span)
                } else {
                    self.session.types.borrow_mut().intern(Type::Void)
                };

                self.env.pop_scope();

                (
                    TypedExprKind::Variable(name),
                    self.session.types.borrow_mut().intern(Type::Function(
                        type_params.clone(),
                        param_types,
                        ret_ty,
                    )),
                )
            } else {
                unreachable!()
            }
        } else {
            self.error(
                span,
                DiagnosticCode::UnknownIdentifier,
                &format!(
                    "Variable '{}' not found.",
                    self.session.interner.borrow().lookup(name)
                ),
            );
            (
                TypedExprKind::Variable(name),
                self.session.types.borrow_mut().intern(Type::Error),
            )
        }
    }

    pub(crate) fn check_assign_expr(
        &mut self,
        name: Symbol,
        value: &Expr<'a>,
        span: Span,
    ) -> (TypedExprKind<'a>, TypeId) {
        let typed_val = self.check_expr(value);
        if let Some(var_type) = self.env.resolve(name) {
            if !self.env.is_mutable(name) {
                self.error(
                    span,
                    DiagnosticCode::ImmutableAssignment,
                    &format!(
                        "Cannot mutate immutable variable '{}'.",
                        self.session.interner.borrow().lookup(name)
                    ),
                )
            }
            if typed_val.ty != var_type
                && typed_val.ty != self.session.types.borrow_mut().intern(Type::Error)
                && var_type != self.session.types.borrow_mut().intern(Type::Error)
                && var_type != self.session.types.borrow_mut().intern(Type::Any)
            {
                self.error(
                    span,
                    DiagnosticCode::TypeMismatch,
                    &format!(
                        "Cannot assign type '{}' to variable of type '{}'.",
                        self.session.format_type(typed_val.ty),
                        self.session.format_type(var_type)
                    ),
                )
            }
        } else {
            self.error(
                span,
                DiagnosticCode::UnknownIdentifier,
                &format!(
                    "Variable '{}' not found.",
                    self.session.interner.borrow().lookup(name)
                ),
            )
        }
        (
            TypedExprKind::Assign {
                name,
                value: self.alloc(typed_val.clone()),
            },
            typed_val.ty,
        )
    }

    pub(crate) fn check_self_ref_expr(&mut self, span: Span) -> (TypedExprKind<'a>, TypeId) {
        if let Some(ty) = self
            .env
            .resolve(self.session.interner.borrow_mut().intern("self"))
        {
            (TypedExprKind::SelfRef, ty)
        } else {
            self.error(
                span,
                DiagnosticCode::TypeMismatch,
                "Cannot use 'self' outside a class.",
            );
            (
                TypedExprKind::SelfRef,
                self.session.types.borrow_mut().intern(Type::Error),
            )
        }
    }

    pub(crate) fn check_optional_get_expr(
        &mut self,
        object: &Expr<'a>,
        name: Symbol,
        span: Span,
    ) -> (TypedExprKind<'a>, TypeId) {
        let typed_obj = self.check_expr(object);
        let ty = match self.get_type(typed_obj.ty) {
            Type::Optional(inner) => {
                if let Type::Instance(class_name) = self.session.types.borrow().get(inner) {
                    if let Some(fields) = self.classes.get(class_name) {
                        if let Some(field_ty) = fields.get(&name) {
                            self.session
                                .types
                                .borrow_mut()
                                .intern(Type::Optional(*field_ty))
                        } else {
                            self.error(
                                span,
                                DiagnosticCode::UnknownIdentifier,
                                &format!(
                                    "Property '{}' not found on '{}'.",
                                    self.session.interner.borrow().lookup(name),
                                    self.session.interner.borrow().lookup(*class_name)
                                ),
                            );
                            self.session.types.borrow_mut().intern(Type::Error)
                        }
                    } else {
                        self.session.types.borrow_mut().intern(Type::Error)
                    }
                } else {
                    self.error(
                        span,
                        DiagnosticCode::TypeMismatch,
                        &format!(
                            "Cannot access property on non-instance optional type '{}'.",
                            self.session.format_type(inner)
                        ),
                    );
                    self.session.types.borrow_mut().intern(Type::Error)
                }
            }
            Type::Error | Type::Any => typed_obj.ty,
            _ => {
                self.error(
                    span,
                    DiagnosticCode::TypeMismatch,
                    &format!(
                        "Optional chaining '?.' requires an optional type, found '{}'.",
                        self.session.format_type(typed_obj.ty)
                    ),
                );
                self.session.types.borrow_mut().intern(Type::Error)
            }
        };
        (
            TypedExprKind::OptionalGet {
                object: self.alloc(typed_obj),
                name,
            },
            ty,
        )
    }

    pub(crate) fn check_index_get_expr(
        &mut self,
        object: &Expr<'a>,
        index: &Expr<'a>,
        span: Span,
    ) -> (TypedExprKind<'a>, TypeId) {
        let typed_obj = self.check_expr(object);
        let typed_idx = self.check_expr(index);
        if typed_idx.ty != self.session.types.borrow_mut().intern(Type::Int)
            && typed_idx.ty != self.session.types.borrow_mut().intern(Type::Error)
        {
            self.error(
                span,
                DiagnosticCode::TypeMismatch,
                &format!(
                    "Array index must be 'Int', found '{}'.",
                    self.session.format_type(typed_idx.ty)
                ),
            );
        }
        let ty = match self.get_type(typed_obj.ty) {
            Type::Array(inner) => inner,
            Type::Error => self.session.types.borrow_mut().intern(Type::Error),
            _ => {
                self.error(
                    span,
                    DiagnosticCode::TypeMismatch,
                    &format!(
                        "Cannot index into non-array type '{}'.",
                        self.session.format_type(typed_obj.ty)
                    ),
                );
                self.session.types.borrow_mut().intern(Type::Error)
            }
        };
        (
            TypedExprKind::IndexGet {
                object: self.alloc(typed_obj),
                index: self.alloc(typed_idx),
            },
            ty,
        )
    }

    pub(crate) fn check_index_set_expr(
        &mut self,
        object: &Expr<'a>,
        index: &Expr<'a>,
        value: &Expr<'a>,
        span: Span,
    ) -> (TypedExprKind<'a>, TypeId) {
        let typed_obj = self.check_expr(object);
        let typed_idx = self.check_expr(index);
        let typed_val = self.check_expr(value);

        if typed_idx.ty != self.session.types.borrow_mut().intern(Type::Int)
            && typed_idx.ty != self.session.types.borrow_mut().intern(Type::Error)
        {
            self.error(
                span,
                DiagnosticCode::TypeMismatch,
                &format!(
                    "Array index must be 'Int', found '{}'.",
                    self.session.format_type(typed_idx.ty)
                ),
            );
        }

        match self.get_type(typed_obj.ty) {
            Type::Array(inner) => {
                if !self.is_assignable(typed_val.ty, inner)
                    && typed_val.ty != self.session.types.borrow_mut().intern(Type::Error)
                    && inner != self.session.types.borrow_mut().intern(Type::Error)
                    && inner != self.session.types.borrow_mut().intern(Type::Any)
                {
                    self.error(
                        span,
                        DiagnosticCode::TypeMismatch,
                        &format!(
                            "Cannot assign type '{}' to array element of type '{}'.",
                            self.session.format_type(typed_val.ty),
                            self.session.format_type(inner)
                        ),
                    );
                }
            }
            Type::Error => {}
            _ => {
                self.error(
                    span,
                    DiagnosticCode::TypeMismatch,
                    &format!(
                        "Cannot index into non-array type '{}'.",
                        self.session.format_type(typed_obj.ty)
                    ),
                );
            }
        }
        (
            TypedExprKind::IndexSet {
                object: self.alloc(typed_obj),
                index: self.alloc(typed_idx),
                value: self.alloc(typed_val.clone()),
            },
            typed_val.ty,
        )
    }

    pub(crate) fn check_get_expr(
        &mut self,
        object: &Expr<'a>,
        name: Symbol,
        span: Span,
    ) -> (TypedExprKind<'a>, TypeId) {
        let mut typed_obj = self.check_expr(object);
        let base_sym = match self.session.types.borrow().get(typed_obj.ty).clone() {
            Type::Class(sym, _)
            | Type::Struct(sym, _)
            | Type::Instance(sym)
            | Type::GenericInstance(sym, _)
            | Type::Enum(sym, _) => sym,
            _ => {
                let formatted = self.session.format_type(typed_obj.ty);
                self.session.interner.borrow_mut().intern(&formatted)
            }
        };

        let (class_name, instance_args) = match self.get_type(typed_obj.ty) {
            Type::Instance(n) => (n, Vec::new()),
            Type::GenericInstance(n, args) => {
                let mut mangled_name = n;
                let mut is_concrete = true;
                let mut is_enum = false;
                for ty in &args {
                    if let Type::Generic(_) = self.get_type(*ty) {
                        is_concrete = false;
                        break;
                    }
                }

                if is_concrete {
                    if let Some(stmt) = self.generic_registry.get_class(n).cloned() {
                        match &stmt.kind {
                            StmtKind::Class { type_params, .. }
                            | StmtKind::Struct { type_params, .. } => {
                                mangled_name =
                                    self.instantiate_generic_class(n, type_params, &args);
                            }
                            StmtKind::Enum { type_params, .. } => {
                                mangled_name =
                                    self.instantiate_generic_class(n, type_params, &args);
                                is_enum = true;
                            }
                            _ => {}
                        }
                    }
                    if is_enum {
                        typed_obj.ty = self
                            .session
                            .types
                            .borrow_mut()
                            .intern(Type::Enum(mangled_name, Vec::new()));
                    } else {
                        typed_obj.ty = self
                            .session
                            .types
                            .borrow_mut()
                            .intern(Type::Instance(mangled_name));
                    }
                }
                (mangled_name, args.clone())
            }
            Type::Interface(n, type_args) => (
                n,
                type_args
                    .iter()
                    .map(|s| self.session.types.borrow_mut().intern(Type::Generic(*s)))
                    .collect(),
            ),
            Type::Class(n, _) | Type::Struct(n, _) | Type::Enum(n, _) => {
                if let Some(static_props) = self.class_static_members.get(&n) {
                    if let Some(prop_ty) = static_props.get(&name) {
                        return (
                            TypedExprKind::Get {
                                object: self.alloc(typed_obj),
                                name,
                                is_static: true,
                            },
                            *prop_ty,
                        );
                    }
                }

                if let Type::Enum(n, _params) = self.get_type(typed_obj.ty) {
                    if let Some(enum_variants) = self.enums.get(&n) {
                        if let Some(variant_ty) = enum_variants.get(&name) {
                            // Currently, we don't have generic arguments provided at the static access site for enum variants,
                            // e.g. `Option.Some`. If they need arguments, it's typically a constructor call.
                            // We just return the generic variant type.
                            return (
                                TypedExprKind::EnumVariant {
                                    enum_name: n,
                                    variant_name: name,
                                },
                                *variant_ty,
                            );
                        }
                    }
                }

                if matches!(self.get_type(typed_obj.ty), Type::Enum(..)) {
                    (n, Vec::new())
                } else {
                    self.error(
                        span,
                        DiagnosticCode::UnknownType,
                        &format!(
                            "Static property '{}' not found on type '{}'.",
                            self.session.interner.borrow().lookup(name),
                            self.session.interner.borrow().lookup(n)
                        ),
                    );
                    return (
                        TypedExprKind::Get {
                            object: self.alloc(typed_obj),
                            name,
                            is_static: true,
                        },
                        self.session.types.borrow_mut().intern(Type::Error),
                    );
                }
            }
            Type::Array(inner) => {
                let array_sym = self.session.interner.borrow_mut().intern("$ArrayExtension");
                let mut type_params = Vec::new();
                if let Some(exts) = self.generic_registry.get_extensions(array_sym) {
                    if let Some(ext) = exts.first() {
                        if let ast::StmtKind::Extension { type_params: tps, .. } = &ext.kind {
                            type_params = tps.clone();
                        }
                    }
                }
                
                let mangled_name = if !type_params.is_empty() {
                    self.instantiate_generic_class(
                        array_sym,
                        &type_params,
                        &[inner],
                    )
                } else {
                    array_sym
                };

                if let Some(ext_methods) = self.extensions.get(&mangled_name) {
                    if let Some(ext_method_ty) = ext_methods.get(&name) {
                        return (
                            TypedExprKind::Get {
                                object: self.alloc(typed_obj),
                                name,
                                is_static: false,
                            },
                            *ext_method_ty,
                        );
                    }
                }

                if typed_obj.ty != self.session.types.borrow_mut().intern(Type::Error) {
                    self.error(
                        span,
                        DiagnosticCode::TypeMismatch,
                        &format!(
                            "Cannot get property '{}' on non-instance type '{}'.",
                            self.session.interner.borrow().lookup(name),
                            self.session.format_type(typed_obj.ty)
                        ),
                    )
                }
                return (
                    TypedExprKind::Get {
                        object: self.alloc(typed_obj),
                        name,
                        is_static: false,
                    },
                    self.session.types.borrow_mut().intern(Type::Error),
                );
            }
            _ => {
                if let Some(ext_methods) = self.extensions.get(&base_sym) {
                    if let Some(ext_method_ty) = ext_methods.get(&name) {
                        return (
                            TypedExprKind::Get {
                                object: self.alloc(typed_obj),
                                name,
                                is_static: false,
                            },
                            *ext_method_ty,
                        );
                    }
                }

                if typed_obj.ty != self.session.types.borrow_mut().intern(Type::Error) {
                    self.error(
                        span,
                        DiagnosticCode::TypeMismatch,
                        &format!(
                            "Cannot get property '{}' on non-instance type '{}'.",
                            self.session.interner.borrow().lookup(name),
                            self.session.format_type(typed_obj.ty)
                        ),
                    )
                }
                return (
                    TypedExprKind::Get {
                        object: self.alloc(typed_obj),
                        name,
                        is_static: false,
                    },
                    self.session.types.borrow_mut().intern(Type::Error),
                );
            }
        };

        let ty = if let Some(class_props) = self.classes.get(&class_name) {
            if let Some(prop_ty) = class_props.get(&name) {
                let mut resolved_ty = *prop_ty;
                if let Some(Type::Class(_, params))
                | Some(Type::Struct(_, params))
                | Some(Type::Enum(_, params)) =
                    self.env.resolve(class_name).map(|id| self.get_type(id))
                {
                    let mut inferred_map = std::collections::HashMap::new();
                    for (i, p) in params.iter().enumerate() {
                        if i < instance_args.len() {
                            inferred_map.insert(*p, instance_args[i]);
                        }
                    }
                    resolved_ty = self.substitute_generics(*prop_ty, &inferred_map);
                }
                resolved_ty
            } else if let Some(enum_variants) = self.enums.get(&class_name) {
                if let Some(variant_ty) = enum_variants.get(&name) {
                    let mut resolved_ty = *variant_ty;
                    // Instantiate generic arguments if present
                    if let Type::Enum(_, params) = self.session.types.borrow().get(typed_obj.ty) {
                        let mut inferred_map = std::collections::HashMap::new();
                        for (i, p) in params.iter().enumerate() {
                            if i < instance_args.len() {
                                inferred_map.insert(*p, instance_args[i]);
                            }
                        }
                        resolved_ty = self.substitute_generics(*variant_ty, &inferred_map);
                    }

                    return (
                        TypedExprKind::EnumVariant {
                            enum_name: class_name,
                            variant_name: name,
                        },
                        resolved_ty,
                    );
                } else {
                    self.error(
                        span,
                        DiagnosticCode::UnknownType,
                        &format!(
                            "Property '{}' not found on enum/class '{}'.",
                            self.session.interner.borrow().lookup(name),
                            self.session.interner.borrow().lookup(class_name)
                        ),
                    );
                    self.session.types.borrow_mut().intern(Type::Error)
                }
            } else {
                if let Some(ext_methods) = self.extensions.get(&base_sym) {
                    if let Some(ext_method_ty) = ext_methods.get(&name) {
                        let mut resolved_ty = *ext_method_ty;
                        let mut inferred_map = None;
                        if let Some(Type::Class(_, class_params))
                        | Some(Type::Struct(_, class_params))
                        | Some(Type::Enum(_, class_params)) =
                            self.env.resolve(base_sym).map(|id| self.get_type(id))
                        {
                            let mut map = std::collections::HashMap::new();
                            for (i, p) in class_params.iter().enumerate() {
                                if i < instance_args.len() {
                                    map.insert(*p, instance_args[i]);
                                }
                            }
                            inferred_map = Some(map);
                        }
                        if let Some(map) = inferred_map {
                            resolved_ty = self.substitute_generics(resolved_ty, &map);
                        }
                        resolved_ty
                    } else {
                        self.error(
                            span,
                            DiagnosticCode::UnknownType,
                            &format!(
                                "Property '{}' not found on class '{}' or its extensions.",
                                self.session.interner.borrow().lookup(name),
                                self.session.interner.borrow().lookup(class_name)
                            ),
                        );
                        self.session.types.borrow_mut().intern(Type::Error)
                    }
                } else {
                    self.error(
                        span,
                        DiagnosticCode::UnknownType,
                        &format!(
                            "Property '{}' not found on class '{}'.",
                            self.session.interner.borrow().lookup(name),
                            self.session.interner.borrow().lookup(class_name)
                        ),
                    );
                    self.session.types.borrow_mut().intern(Type::Error)
                }
            }
        } else if let Some(interface_props) = self.interfaces.get(&class_name) {
            if let Some(prop_ty) = interface_props.get(&name) {
                *prop_ty
            } else {
                if let Some(ext_methods) = self.extensions.get(&base_sym) {
                    if let Some(ext_method_ty) = ext_methods.get(&name) {
                        let mut resolved_ty = *ext_method_ty;
                        let mut inferred_map = None;
                        if let Some(Type::Class(_, class_params))
                        | Some(Type::Struct(_, class_params))
                        | Some(Type::Enum(_, class_params)) =
                            self.env.resolve(base_sym).map(|id| self.get_type(id))
                        {
                            let mut map = std::collections::HashMap::new();
                            for (i, p) in class_params.iter().enumerate() {
                                if i < instance_args.len() {
                                    map.insert(*p, instance_args[i]);
                                }
                            }
                            inferred_map = Some(map);
                        }
                        if let Some(map) = inferred_map {
                            resolved_ty = self.substitute_generics(resolved_ty, &map);
                        }
                        resolved_ty
                    } else {
                        self.error(
                            span,
                            DiagnosticCode::UnknownType,
                            &format!(
                                "Property '{}' not found on interface '{}' or its extensions.",
                                self.session.interner.borrow().lookup(name),
                                self.session.interner.borrow().lookup(class_name)
                            ),
                        );
                        self.session.types.borrow_mut().intern(Type::Error)
                    }
                } else {
                    self.error(
                        span,
                        DiagnosticCode::UnknownType,
                        &format!(
                            "Property '{}' not found on interface '{}'.",
                            self.session.interner.borrow().lookup(name),
                            self.session.interner.borrow().lookup(class_name)
                        ),
                    );
                    self.session.types.borrow_mut().intern(Type::Error)
                }
            }
        } else {
            if let Some(ext_methods) = self.extensions.get(&base_sym) {
                if let Some(ext_method_ty) = ext_methods.get(&name) {
                    let mut resolved_ty = *ext_method_ty;
                    let mut inferred_map = None;
                    if let Some(Type::Class(_, class_params))
                    | Some(Type::Struct(_, class_params))
                    | Some(Type::Enum(_, class_params)) =
                        self.env.resolve(base_sym).map(|id| self.get_type(id))
                    {
                        let mut map = std::collections::HashMap::new();
                        for (i, p) in class_params.iter().enumerate() {
                            if i < instance_args.len() {
                                map.insert(*p, instance_args[i]);
                            }
                        }
                        inferred_map = Some(map);
                    }
                    if let Some(map) = inferred_map {
                        resolved_ty = self.substitute_generics(resolved_ty, &map);
                    }
                    resolved_ty
                } else {
                    self.error(
                        span,
                        DiagnosticCode::UnknownType,
                        &format!(
                            "Property '{}' not found on type '{}' or its extensions.",
                            self.session.interner.borrow().lookup(name),
                            self.session.interner.borrow().lookup(class_name)
                        ),
                    );
                    self.session.types.borrow_mut().intern(Type::Error)
                }
            } else {
                self.error(
                    span,
                    DiagnosticCode::UnknownType,
                    &format!(
                        "Type '{}' not found.",
                        self.session.interner.borrow().lookup(class_name)
                    ),
                );
                self.session.types.borrow_mut().intern(Type::Error)
            }
        };
        (
            TypedExprKind::Get {
                object: self.alloc(typed_obj),
                name,
                is_static: false, // Wait! I should check what line 761 is doing. Let me assume it's instance because it's at the end of the file, likely fallback. Wait, let me view it first!
            },
            ty,
        )
    }

    pub(crate) fn check_set_expr(
        &mut self,
        object: &Expr<'a>,
        name: Symbol,
        value: &Expr<'a>,
        span: Span,
    ) -> (TypedExprKind<'a>, TypeId) {
        let typed_obj = self.check_expr(object);
        let typed_val = self.check_expr(value);

        let (class_name, instance_args) = match self.get_type(typed_obj.ty) {
            Type::Instance(n) => (n, Vec::new()),
            Type::GenericInstance(n, args) => (n, args.clone()),
            Type::Interface(n, type_args) => (
                n,
                type_args
                    .iter()
                    .map(|s| self.session.types.borrow_mut().intern(Type::Generic(*s)))
                    .collect(),
            ),
            Type::Class(n, _) | Type::Struct(n, _) | Type::Enum(n, _) => {
                let static_lookup = self.class_static_members.get(&n).and_then(|props| props.get(&name).copied());
                let mut is_immutable = false;
                if static_lookup.is_some() {
                    if let Some(muts) = self.class_static_mutables.get(&n) {
                        if let Some(&is_mut) = muts.get(&name) {
                            if !is_mut {
                                is_immutable = true;
                            }
                        }
                    }
                }

                if let Some(prop_ty) = static_lookup {
                    if is_immutable {
                            self.error(
                                span,
                                DiagnosticCode::ImmutableAssignment,
                                &format!(
                                    "Cannot mutate immutable static property '{}'.",
                                    self.session.interner.borrow().lookup(name)
                                ),
                            )
                        }
                        
                        if !self.is_assignable(typed_val.ty, prop_ty)
                            && typed_val.ty != self.session.types.borrow_mut().intern(Type::Error)
                            && prop_ty != self.session.types.borrow_mut().intern(Type::Error)
                            && prop_ty != self.session.types.borrow_mut().intern(Type::Any)
                        {
                            self.error(
                                span,
                                DiagnosticCode::TypeMismatch,
                                &format!(
                                    "Cannot assign type '{}' to static property of type '{}'.",
                                    self.session.format_type(typed_val.ty),
                                    self.session.format_type(prop_ty)
                                ),
                            );
                        }
                    return (
                        TypedExprKind::Set {
                            object: self.alloc(typed_obj),
                            name,
                            value: self.alloc(typed_val.clone()),
                            is_static: true,
                        },
                        typed_val.ty,
                    );
                }
                
                if matches!(self.get_type(typed_obj.ty), Type::Enum(..)) {
                    (n, Vec::new())
                } else {
                    self.error(
                        span,
                        DiagnosticCode::UnknownType,
                        &format!(
                            "Static property '{}' not found on type '{}'.",
                            self.session.interner.borrow().lookup(name),
                            self.session.interner.borrow().lookup(n)
                        ),
                    );
                    return (
                        TypedExprKind::Set {
                            object: self.alloc(typed_obj),
                            name,
                            value: self.alloc(typed_val.clone()),
                            is_static: true,
                        },
                        self.session.types.borrow_mut().intern(Type::Error),
                    );
                }
            }
            _ => {
                if typed_obj.ty != self.session.types.borrow_mut().intern(Type::Error) {
                    self.error(
                        span,
                        DiagnosticCode::TypeMismatch,
                        &format!(
                            "Cannot set property '{}' on non-instance type '{}'.",
                            self.session.interner.borrow().lookup(name),
                            self.session.format_type(typed_obj.ty)
                        ),
                    )
                }
                return (
                    TypedExprKind::Set {
                        object: self.alloc(typed_obj),
                        name,
                        value: self.alloc(typed_val.clone()),
                        is_static: false,
                    },
                    typed_val.ty,
                );
            }
        };

        if let Some(class_props) = self.classes.get(&class_name) {
            if let Some(prop_ty) = class_props.get(&name) {
                let mut resolved_ty = *prop_ty;
                if let Some(Type::Class(_, params)) | Some(Type::Struct(_, params)) =
                    self.env.resolve(class_name).map(|id| self.get_type(id))
                {
                    let mut inferred_map = std::collections::HashMap::new();
                    for (i, p) in params.iter().enumerate() {
                        if i < instance_args.len() {
                            inferred_map.insert(*p, instance_args[i]);
                        }
                    }
                    resolved_ty = self.substitute_generics(*prop_ty, &inferred_map);
                }

                if let Some(muts) = self.class_mutables.get(&class_name)
                    && let Some(&is_mut) = muts.get(&name)
                    && !is_mut
                {
                    self.error(
                        span,
                        DiagnosticCode::ImmutableAssignment,
                        &format!(
                            "Cannot mutate immutable property '{}'.",
                            self.session.interner.borrow().lookup(name)
                        ),
                    )
                }
                if !self.is_assignable(typed_val.ty, resolved_ty)
                    && typed_val.ty != self.session.types.borrow_mut().intern(Type::Error)
                    && resolved_ty != self.session.types.borrow_mut().intern(Type::Error)
                    && resolved_ty != self.session.types.borrow_mut().intern(Type::Any)
                {
                    self.error(
                        span,
                        DiagnosticCode::TypeMismatch,
                        &format!(
                            "Cannot assign type '{}' to property of type '{}'.",
                            self.session.format_type(typed_val.ty),
                            self.session.format_type(resolved_ty)
                        ),
                    )
                }
            } else {
                self.error(
                    span,
                    DiagnosticCode::UnknownType,
                    &format!(
                        "Property '{}' not found on class '{}'.",
                        self.session.interner.borrow().lookup(name),
                        self.session.interner.borrow().lookup(class_name)
                    ),
                )
            }
        }
        (
            TypedExprKind::Set {
                object: self.alloc(typed_obj),
                name,
                value: self.alloc(typed_val.clone()),
                is_static: false,
            },
            typed_val.ty,
        )
    }
}
