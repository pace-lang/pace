use super::super::*;
use session::types::{Type, TypeId};
use session::interner::Symbol;

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
        let typed_obj = self.check_expr(object);

        let (class_name, instance_args) = match self.get_type(typed_obj.ty) {
            Type::Instance(n) => (n, Vec::new()),
            Type::GenericInstance(n, args) => (n, args.clone()),
            Type::Interface(n, type_args) => (n, type_args.iter().map(|s| self.session.types.borrow_mut().intern(Type::Generic(*s))).collect()),
            Type::Enum(n, _args) => (n, Vec::new()),
            _ => {
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
                    },
                    self.session.types.borrow_mut().intern(Type::Error),
                );
            }
        };

        let ty = if let Some(class_props) = self.classes.get(&class_name) {
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
                resolved_ty
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
        } else if let Some(interface_props) = self.interfaces.get(&class_name) {
            if let Some(prop_ty) = interface_props.get(&name) {
                *prop_ty
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
        } else if let Some(enum_variants) = self.enums.get(&class_name) {
            if let Some(variant_ty) = enum_variants.get(&name) {
                let mut resolved_ty = *variant_ty;
                // Instantiate generic arguments if present
                if let Type::Enum(_, params) = self.session.types.borrow().get(typed_obj.ty)
                {
                    let mut inferred_map = std::collections::HashMap::new();
                    for (i, p) in params.iter().enumerate() {
                        if i < instance_args.len() {
                            inferred_map.insert(*p, instance_args[i]);
                        }
                    }
                    resolved_ty = self.substitute_generics(*variant_ty, &inferred_map);
                }

                // If it's a unit variant (no params), it evaluates to the enum type directly
                if let Type::EnumVariantConstructor(_, _, _, params, ret_ty) =
                    self.session.types.borrow().get(resolved_ty)
                    && params.is_empty()
                {
                    resolved_ty = *ret_ty;
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
                        "Variant '{}' not found in enum '{}'.",
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
        };
        (
            TypedExprKind::Get {
                object: self.alloc(typed_obj),
                name,
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
            Type::Interface(n, type_args) => (n, type_args.iter().map(|s| self.session.types.borrow_mut().intern(Type::Generic(*s))).collect()),
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
            },
            typed_val.ty,
        )
    }
}
