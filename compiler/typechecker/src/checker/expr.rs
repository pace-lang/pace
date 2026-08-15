use super::*;
use session::types::Type;

impl<'a> TypeChecker<'a> {
    pub(crate) fn check_expr(&mut self, expr: &Expr<'a>) -> TypedExpr<'a> {
        self.check_expr_with_expected(expr, None)
    }

    pub(crate) fn check_expr_with_expected(
        &mut self,
        expr: &Expr<'a>,
        expected_ty: Option<TypeId>,
    ) -> TypedExpr<'a> {
        let (kind, ty) = match &expr.kind {
            ExprKind::Integer(v) => (
                TypedExprKind::Integer(*v),
                self.session.types.borrow_mut().intern(Type::Int),
            ),
            ExprKind::Float(v) => (
                TypedExprKind::Float(*v),
                self.session.types.borrow_mut().intern(Type::Float),
            ),
            ExprKind::String(v) => (
                TypedExprKind::String(*v),
                self.session.types.borrow_mut().intern(Type::String),
            ),
            ExprKind::InterpolatedString(pieces) => {
                let mut typed_pieces = Vec::new();
                for piece in pieces {
                    let typed_piece = self.check_expr(piece);
                    match self.get_type(typed_piece.ty) {
                        Type::Int | Type::Float | Type::String | Type::Boolean | Type::Error => {}
                        _ => {
                            self.error(piece.span, DiagnosticCode::TypeMismatch, &format!("Cannot interpolate type '{}'. Only Int, Float, String, and Boolean are supported.", self.session.format_type(typed_piece.ty)));
                        }
                    }
                    typed_pieces.push(typed_piece);
                }
                (
                    TypedExprKind::InterpolatedString(typed_pieces),
                    self.session.types.borrow_mut().intern(Type::String),
                )
            }
            ExprKind::Boolean(v) => (
                TypedExprKind::Boolean(*v),
                self.session.types.borrow_mut().intern(Type::Boolean),
            ),
            ExprKind::Null => (
                TypedExprKind::Null,
                self.session.types.borrow_mut().intern(Type::Null),
            ),
            ExprKind::Variable(name) => {
                if let Some(ty) = self.env.resolve(*name) {
                    (TypedExprKind::Variable(*name), ty)
                } else if let Some(generic_stmt) = self.generic_registry.get_class(*name).cloned() {
                    if let ast::StmtKind::Class { type_params, .. } = &generic_stmt.kind {
                        (
                            TypedExprKind::Variable(*name),
                            self.session
                                .types
                                .borrow_mut()
                                .intern(Type::Class(*name, type_params.clone())),
                        )
                    } else {
                        unreachable!()
                    }
                } else if let Some(generic_stmt) =
                    self.generic_registry.get_function(*name).cloned()
                {
                    if let ast::StmtKind::Func {
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
                            param_types.push(self.parse_type(ty, expr.span));
                        }
                        let ret_ty = if let Some(ty) = return_type {
                            self.parse_type(ty, expr.span)
                        } else {
                            self.session.types.borrow_mut().intern(Type::Void)
                        };

                        self.env.pop_scope();

                        (
                            TypedExprKind::Variable(*name),
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
                        expr.span,
                        DiagnosticCode::UnknownIdentifier,
                        &format!(
                            "Variable '{}' not found.",
                            self.session.interner.borrow().lookup(*name)
                        ),
                    );
                    (
                        TypedExprKind::Variable(*name),
                        self.session.types.borrow_mut().intern(Type::Error),
                    )
                }
            }
            ExprKind::Assign { name, value } => {
                let typed_val = self.check_expr(value);
                if let Some(var_type) = self.env.resolve(*name) {
                    if !self.env.is_mutable(*name) {
                        self.error(
                            expr.span,
                            DiagnosticCode::ImmutableAssignment,
                            &format!(
                                "Cannot mutate immutable variable '{}'.",
                                self.session.interner.borrow().lookup(*name)
                            ),
                        )
                    }
                    if typed_val.ty != var_type
                        && typed_val.ty != self.session.types.borrow_mut().intern(Type::Error)
                        && var_type != self.session.types.borrow_mut().intern(Type::Error)
                        && var_type != self.session.types.borrow_mut().intern(Type::Any)
                    {
                        self.error(
                            expr.span,
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
                        expr.span,
                        DiagnosticCode::UnknownIdentifier,
                        &format!(
                            "Variable '{}' not found.",
                            self.session.interner.borrow().lookup(*name)
                        ),
                    )
                }
                (
                    TypedExprKind::Assign {
                        name: *name,
                        value: self.alloc(typed_val.clone()),
                    },
                    typed_val.ty,
                )
            }
            ExprKind::SelfRef => {
                if let Some(ty) = self
                    .env
                    .resolve(self.session.interner.borrow_mut().intern("self"))
                {
                    (TypedExprKind::SelfRef, ty)
                } else {
                    self.error(
                        expr.span,
                        DiagnosticCode::TypeMismatch,
                        "Cannot use 'self' outside a class.",
                    );
                    (
                        TypedExprKind::SelfRef,
                        self.session.types.borrow_mut().intern(Type::Error),
                    )
                }
            }
            ExprKind::ForceUnwrap(inner) => {
                let typed_inner = self.check_expr(inner);
                let ty = match self.get_type(typed_inner.ty) {
                    Type::Optional(inner_inner) => inner_inner,
                    Type::Null => {
                        self.error(
                            expr.span,
                            DiagnosticCode::TypeMismatch,
                            "Cannot force unwrap a null literal.",
                        );
                        self.session.types.borrow_mut().intern(Type::Error)
                    }
                    Type::Error | Type::Any => typed_inner.ty,
                    _ => {
                        self.error(
                            expr.span,
                            DiagnosticCode::TypeMismatch,
                            &format!(
                                "Cannot force unwrap non-optional type '{}'.",
                                self.session.format_type(typed_inner.ty)
                            ),
                        );
                        typed_inner.ty
                    }
                };
                (TypedExprKind::ForceUnwrap(self.alloc(typed_inner)), ty)
            }
            ExprKind::OptionalGet { object, name } => {
                let typed_obj = self.check_expr(object);
                let ty = match self.get_type(typed_obj.ty) {
                    Type::Optional(inner) => {
                        if let Type::Instance(class_name) = self.session.types.borrow().get(inner) {
                            if let Some(fields) = self.classes.get(class_name) {
                                if let Some(field_ty) = fields.get(name) {
                                    self.session
                                        .types
                                        .borrow_mut()
                                        .intern(Type::Optional(*field_ty))
                                } else {
                                    self.error(
                                        expr.span,
                                        DiagnosticCode::UnknownIdentifier,
                                        &format!(
                                            "Property '{}' not found on '{}'.",
                                            self.session.interner.borrow().lookup(*name),
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
                                expr.span,
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
                            expr.span,
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
                        name: *name,
                    },
                    ty,
                )
            }
            ExprKind::NullCoalesce { left, right } => {
                let typed_left = self.check_expr(left);
                let typed_right = self.check_expr(right);

                match self.get_type(typed_left.ty) {
                    Type::Optional(inner) => {
                        let expected = inner;
                        let is_valid = typed_right.ty == expected
                            || typed_right.ty == typed_left.ty
                            || typed_right.ty
                                == self.session.types.borrow_mut().intern(Type::Error)
                            || typed_left.ty == self.session.types.borrow_mut().intern(Type::Error);

                        if !is_valid {
                            self.error(
                                expr.span,
                                DiagnosticCode::TypeMismatch,
                                &format!(
                                    "Cannot coalesce type '{}' with '{}'.",
                                    self.session.format_type(typed_left.ty),
                                    self.session.format_type(typed_right.ty)
                                ),
                            );
                        }

                        (
                            TypedExprKind::NullCoalesce {
                                left: self.alloc(typed_left),
                                right: self.alloc(typed_right.clone()),
                            },
                            typed_right.ty,
                        )
                    }
                    Type::Null => (
                        TypedExprKind::NullCoalesce {
                            left: self.alloc(typed_left),
                            right: self.alloc(typed_right.clone()),
                        },
                        typed_right.ty,
                    ),
                    Type::Error | Type::Any => (
                        TypedExprKind::NullCoalesce {
                            left: self.alloc(typed_left),
                            right: self.alloc(typed_right.clone()),
                        },
                        typed_right.ty,
                    ),
                    _ => {
                        self.error(
                            expr.span,
                            DiagnosticCode::TypeMismatch,
                            &format!(
                                "Left operand of '??' must be an optional type, found '{}'.",
                                self.session.format_type(typed_left.ty)
                            ),
                        );
                        (
                            TypedExprKind::NullCoalesce {
                                left: self.alloc(typed_left),
                                right: self.alloc(typed_right.clone()),
                            },
                            typed_right.ty,
                        )
                    }
                }
            }
            ExprKind::NullCoalesceAssign { left, right } => {
                let typed_left = self.check_expr(left);
                let typed_right = self.check_expr(right);

                match self.get_type(typed_left.ty) {
                    Type::Optional(inner) => {
                        let expected = inner;
                        if let TypedExprKind::Variable(ref left_name) = typed_left.kind
                            && !self.env.is_mutable(*left_name) {
                                self.error(
                                    expr.span,
                                    DiagnosticCode::ImmutableAssignment,
                                    &format!(
                                        "Cannot mutate immutable variable '{}'.",
                                        self.session.interner.borrow().lookup(*left_name)
                                    ),
                                )
                            }

                        let is_valid = typed_right.ty == expected
                            || typed_right.ty == typed_left.ty
                            || typed_right.ty
                                == self.session.types.borrow_mut().intern(Type::Error)
                            || typed_left.ty == self.session.types.borrow_mut().intern(Type::Error);

                        if !is_valid {
                            self.error(
                                expr.span,
                                DiagnosticCode::TypeMismatch,
                                &format!(
                                    "Cannot assign type '{}' to variable of type '{}'.",
                                    self.session.format_type(typed_right.ty),
                                    self.session.format_type(typed_left.ty)
                                ),
                            );
                        }

                        (
                            TypedExprKind::NullCoalesceAssign {
                                left: self.alloc(typed_left.clone()),
                                right: self.alloc(typed_right),
                            },
                            typed_left.ty,
                        )
                    }
                    Type::Error | Type::Any => (
                        TypedExprKind::NullCoalesceAssign {
                            left: self.alloc(typed_left.clone()),
                            right: self.alloc(typed_right),
                        },
                        typed_left.ty,
                    ),
                    _ => {
                        self.error(
                            expr.span,
                            DiagnosticCode::TypeMismatch,
                            &format!(
                                "Left operand of '??=' must be an optional type, found '{}'.",
                                self.session.format_type(typed_left.ty)
                            ),
                        );
                        (
                            TypedExprKind::NullCoalesceAssign {
                                left: self.alloc(typed_left.clone()),
                                right: self.alloc(typed_right),
                            },
                            typed_left.ty,
                        )
                    }
                }
            }
            ExprKind::Array(elements) => {
                if elements.is_empty() {
                    self.error(
                        expr.span,
                        DiagnosticCode::TypeMismatch,
                        "Cannot infer type of empty array literal.",
                    );
                    (
                        TypedExprKind::Array(Vec::new()),
                        self.session.types.borrow_mut().intern(Type::Error),
                    )
                } else {
                    let mut typed_elements = Vec::new();
                    let first_typed = self.check_expr(&elements[0]);
                    let elem_type = first_typed.ty;
                    typed_elements.push(first_typed);

                    for elem in elements.iter().skip(1) {
                        let next_typed = self.check_expr(elem);
                        if next_typed.ty != elem_type
                            && next_typed.ty != self.session.types.borrow_mut().intern(Type::Error)
                            && elem_type != self.session.types.borrow_mut().intern(Type::Error)
                        {
                            self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Array elements have inconsistent types: expected '{}', found '{}'.", self.session.format_type(elem_type), self.session.format_type(next_typed.ty)));
                        }
                        typed_elements.push(next_typed);
                    }
                    (
                        TypedExprKind::Array(typed_elements),
                        self.session
                            .types
                            .borrow_mut()
                            .intern(Type::Array(elem_type)),
                    )
                }
            }
            ExprKind::ArrayRepeat { value, count } => {
                let typed_value = self.check_expr(value);
                let typed_count = self.check_expr(count);
                if typed_count.ty != self.session.types.borrow_mut().intern(Type::Int)
                    && typed_count.ty != self.session.types.borrow_mut().intern(Type::Error)
                {
                    self.error(
                        expr.span,
                        DiagnosticCode::TypeMismatch,
                        &format!(
                            "Array repeat count must be 'Int', found '{}'.",
                            self.session.format_type(typed_count.ty)
                        ),
                    );
                }
                let ty = self
                    .session
                    .types
                    .borrow_mut()
                    .intern(Type::Array(typed_value.ty));
                (
                    TypedExprKind::ArrayRepeat {
                        value: self.alloc(typed_value),
                        count: self.alloc(typed_count),
                    },
                    ty,
                )
            }
            ExprKind::ListComprehension {
                expr: mapped_expr,
                item_name,
                iterator,
            } => {
                let typed_iterator = self.check_expr(iterator);

                let item_type = match self.get_type(typed_iterator.ty) {
                    Type::Range => self.session.types.borrow_mut().intern(Type::Int),
                    Type::Array(inner) => inner,
                    Type::Error => self.session.types.borrow_mut().intern(Type::Error),
                    _ => {
                        self.error(
                            expr.span,
                            DiagnosticCode::TypeMismatch,
                            &format!(
                                "Cannot iterate over non-iterable type '{}'.",
                                self.session.format_type(typed_iterator.ty)
                            ),
                        );
                        self.session.types.borrow_mut().intern(Type::Error)
                    }
                };

                self.env.push_scope();
                self.env.declare_var(*item_name, item_type, false);
                let typed_expr = self.check_expr(mapped_expr);
                self.env.pop_scope();

                let ty = self
                    .session
                    .types
                    .borrow_mut()
                    .intern(Type::Array(typed_expr.ty));
                (
                    TypedExprKind::ListComprehension {
                        expr: self.alloc(typed_expr),
                        item_name: *item_name,
                        iterator: self.alloc(typed_iterator),
                    },
                    ty,
                )
            }
            ExprKind::IndexGet { object, index } => {
                let typed_obj = self.check_expr(object);
                let typed_idx = self.check_expr(index);
                if typed_idx.ty != self.session.types.borrow_mut().intern(Type::Int)
                    && typed_idx.ty != self.session.types.borrow_mut().intern(Type::Error)
                {
                    self.error(
                        expr.span,
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
                            expr.span,
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
            ExprKind::IndexSet {
                object,
                index,
                value,
            } => {
                let typed_obj = self.check_expr(object);
                let typed_idx = self.check_expr(index);
                let typed_val = self.check_expr(value);

                if typed_idx.ty != self.session.types.borrow_mut().intern(Type::Int)
                    && typed_idx.ty != self.session.types.borrow_mut().intern(Type::Error)
                {
                    self.error(
                        expr.span,
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
                                expr.span,
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
                            expr.span,
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
            ExprKind::Get { object, name } => {
                let typed_obj = self.check_expr(object);

                let (class_name, instance_args) = match self.get_type(typed_obj.ty) {
                    Type::Instance(n) => (n, Vec::new()),
                    Type::GenericInstance(n, args) => (n, args.clone()),
                    Type::Interface(n) => (n, Vec::new()),
                    Type::Enum(n, _args) => (n, Vec::new()),
                    _ => {
                        if typed_obj.ty != self.session.types.borrow_mut().intern(Type::Error) {
                            self.error(
                                expr.span,
                                DiagnosticCode::TypeMismatch,
                                &format!(
                                    "Cannot get property '{}' on non-instance type '{}'.",
                                    self.session.interner.borrow().lookup(*name),
                                    self.session.format_type(typed_obj.ty)
                                ),
                            )
                        }
                        return TypedExpr::new(
                            TypedExprKind::Get {
                                object: self.alloc(typed_obj),
                                name: *name,
                            },
                            self.session.types.borrow_mut().intern(Type::Error),
                            expr.span,
                        );
                    }
                };

                let ty = if let Some(class_props) = self.classes.get(&class_name) {
                    if let Some(prop_ty) = class_props.get(name) {
                        let mut resolved_ty = *prop_ty;
                        if let Some(Type::Class(_, params)) =
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
                            expr.span,
                            DiagnosticCode::UnknownType,
                            &format!(
                                "Property '{}' not found on class '{}'.",
                                self.session.interner.borrow().lookup(*name),
                                self.session.interner.borrow().lookup(class_name)
                            ),
                        );
                        self.session.types.borrow_mut().intern(Type::Error)
                    }
                } else if let Some(interface_props) = self.interfaces.get(&class_name) {
                    if let Some(prop_ty) = interface_props.get(name) {
                        *prop_ty
                    } else {
                        self.error(
                            expr.span,
                            DiagnosticCode::UnknownType,
                            &format!(
                                "Property '{}' not found on interface '{}'.",
                                self.session.interner.borrow().lookup(*name),
                                self.session.interner.borrow().lookup(class_name)
                            ),
                        );
                        self.session.types.borrow_mut().intern(Type::Error)
                    }
                } else if let Some(enum_variants) = self.enums.get(&class_name) {
                    if let Some(variant_ty) = enum_variants.get(name) {
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

                        return TypedExpr::new(
                            TypedExprKind::EnumVariant {
                                enum_name: class_name,
                                variant_name: *name,
                            },
                            resolved_ty,
                            expr.span,
                        );
                    } else {
                        self.error(
                            expr.span,
                            DiagnosticCode::UnknownType,
                            &format!(
                                "Variant '{}' not found in enum '{}'.",
                                self.session.interner.borrow().lookup(*name),
                                self.session.interner.borrow().lookup(class_name)
                            ),
                        );
                        self.session.types.borrow_mut().intern(Type::Error)
                    }
                } else {
                    self.error(
                        expr.span,
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
                        name: *name,
                    },
                    ty,
                )
            }
            ExprKind::Set {
                object,
                name,
                value,
            } => {
                let typed_obj = self.check_expr(object);
                let typed_val = self.check_expr(value);

                let (class_name, instance_args) = match self.get_type(typed_obj.ty) {
                    Type::Instance(n) => (n, Vec::new()),
                    Type::GenericInstance(n, args) => (n, args.clone()),
                    Type::Interface(n) => (n, Vec::new()),
                    _ => {
                        if typed_obj.ty != self.session.types.borrow_mut().intern(Type::Error) {
                            self.error(
                                expr.span,
                                DiagnosticCode::TypeMismatch,
                                &format!(
                                    "Cannot set property '{}' on non-instance type '{}'.",
                                    self.session.interner.borrow().lookup(*name),
                                    self.session.format_type(typed_obj.ty)
                                ),
                            )
                        }
                        return TypedExpr::new(
                            TypedExprKind::Set {
                                object: self.alloc(typed_obj),
                                name: *name,
                                value: self.alloc(typed_val.clone()),
                            },
                            typed_val.ty,
                            expr.span,
                        );
                    }
                };

                if let Some(class_props) = self.classes.get(&class_name) {
                    if let Some(prop_ty) = class_props.get(name) {
                        let mut resolved_ty = *prop_ty;
                        if let Some(Type::Class(_, params)) =
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
                            && let Some(&is_mut) = muts.get(name)
                                && !is_mut {
                                    self.error(
                                        expr.span,
                                        DiagnosticCode::ImmutableAssignment,
                                        &format!(
                                            "Cannot mutate immutable property '{}'.",
                                            self.session.interner.borrow().lookup(*name)
                                        ),
                                    )
                                }
                        if !self.is_assignable(typed_val.ty, resolved_ty)
                            && typed_val.ty != self.session.types.borrow_mut().intern(Type::Error)
                            && resolved_ty != self.session.types.borrow_mut().intern(Type::Error)
                            && resolved_ty != self.session.types.borrow_mut().intern(Type::Any)
                        {
                            self.error(
                                expr.span,
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
                            expr.span,
                            DiagnosticCode::UnknownType,
                            &format!(
                                "Property '{}' not found on class '{}'.",
                                self.session.interner.borrow().lookup(*name),
                                self.session.interner.borrow().lookup(class_name)
                            ),
                        )
                    }
                }
                (
                    TypedExprKind::Set {
                        object: self.alloc(typed_obj),
                        name: *name,
                        value: self.alloc(typed_val.clone()),
                    },
                    typed_val.ty,
                )
            }
            ExprKind::Grouping(inner) => {
                let typed_inner = self.check_expr(inner);
                let ty = typed_inner.ty;
                (TypedExprKind::Grouping(self.alloc(typed_inner)), ty)
            }
            ExprKind::Match { value, arms } => {
                let typed_value = self.check_expr(value);
                let mut typed_arms = Vec::new();
                let mut common_return_type = None;

                for arm in arms {
                    self.env.push_scope();

                    // Declare bindings in scope
                    match &arm.pattern {
                        ast::Pattern::Wildcard => {}
                        ast::Pattern::Variant { path, bindings } => {
                            if let Some(binds) = bindings {
                                // Extract actual types from the variant
                                let mut extracted_types = Vec::new();

                                let mut enum_name_opt = None;
                                let mut type_args = Vec::new();
                                match self.get_type(typed_value.ty) {
                                    Type::GenericInstance(name, args) => {
                                        enum_name_opt = Some(name);
                                        type_args = args.clone();
                                    }
                                    Type::Instance(name) => {
                                        enum_name_opt = Some(name);
                                    }
                                    _ => {}
                                }

                                if let Some(enum_name) = enum_name_opt
                                    && let Some(variants) = self.enums.get(&enum_name)
                                {
                                    let variant_name = path.last().copied().unwrap_or_else(|| {
                                        self.session.interner.borrow_mut().intern("")
                                    });
                                    if let Some(Type::EnumVariantConstructor(
                                        _,
                                        _,
                                        func_type_params,
                                        param_types,
                                        _,
                                    )) =
                                        variants.get(&variant_name).map(|v_ty| self.get_type(*v_ty))
                                    {
                                        // Substitute generics
                                        let mut replacements = std::collections::HashMap::new();
                                        for (tp, actual) in
                                            func_type_params.iter().zip(type_args.iter())
                                        {
                                            replacements.insert(*tp, *actual);
                                        }
                                        for pt in param_types {
                                            extracted_types
                                                .push(self.substitute_generics(pt, &replacements));
                                        }
                                    }
                                }

                                for (i, bind) in binds.iter().enumerate() {
                                    if self.session.interner.borrow().lookup(*bind) != "_" {
                                        let bind_ty = extracted_types.get(i).cloned().unwrap_or(
                                            self.session.types.borrow_mut().intern(Type::Any),
                                        );
                                        self.env.declare_var(*bind, bind_ty, false);
                                    }
                                }
                            }
                        }
                    }

                    let typed_body = self.check_expr(arm.body);
                    self.env.pop_scope();

                    if let Some(ref crt) = common_return_type {
                        if !self.is_assignable(typed_body.ty, *crt)
                            && typed_body.ty != self.session.types.borrow_mut().intern(Type::Error)
                            && *crt != self.session.types.borrow_mut().intern(Type::Error)
                        {
                            if self.is_assignable(*crt, typed_body.ty) {
                                // Promote crt
                                common_return_type = Some(typed_body.ty);
                            } else {
                                self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Match arms have incompatible return types. Expected '{}', found '{}'.", self.session.format_type(*crt), self.session.format_type(typed_body.ty)));
                            }
                        }
                    } else {
                        common_return_type = Some(typed_body.ty);
                    }

                    typed_arms.push(ast::TypedMatchArm {
                        pattern: arm.pattern.clone(),
                        body: self.alloc(typed_body),
                    });
                }

                // Exhaustiveness checking should happen here.

                let ty = common_return_type
                    .unwrap_or(self.session.types.borrow_mut().intern(Type::Void));
                (
                    TypedExprKind::Match {
                        value: self.alloc(typed_value),
                        arms: typed_arms,
                    },
                    ty,
                )
            }
            ExprKind::Call {
                callee,
                type_args,
                arguments,
            } => {
                let mut typed_callee = self.check_expr(callee);

                let mut expected_param_types = None;
                match self.session.types.borrow().get(typed_callee.ty) {
                    Type::Function(_, param_types, _) => {
                        expected_param_types = Some(param_types.clone());
                    }
                    Type::OverloadedFunction(_variants) => {
                        // For overloaded functions, we'll try to infer based on passed arguments below.
                        // We don't have a single expected param list.
                    }
                    Type::EnumVariantConstructor(_, _, _, param_types, _) => {
                        expected_param_types = Some(param_types.clone());
                    }
                    Type::Class(class_name, _) => {
                        if let Some(props) = self.classes.get(class_name)
                            && let Some(Type::Function(_, param_types, _)) = props
                                .get(&self.session.interner.borrow_mut().intern("init"))
                                .map(|id| self.get_type(*id))
                        {
                            expected_param_types = Some(param_types.clone());
                        }
                    }
                    _ => {}
                }

                let mut typed_args = Vec::new();
                let mut arg_types = Vec::new();
                for (i, arg) in arguments.iter().enumerate() {
                    let expected_arg_ty = expected_param_types.as_ref().and_then(|pt| pt.get(i));
                    let typed_arg = self.check_expr_with_expected(arg, expected_arg_ty.copied());
                    arg_types.push(typed_arg.ty);
                    typed_args.push(typed_arg);
                }

                let ty = match self.get_type(typed_callee.ty) {
                    Type::BuiltinFunc => self.session.types.borrow_mut().intern(Type::Void),
                    Type::OverloadedFunction(variants) => {
                        let mut matched_variant = None;
                        for (mangled_name, ty) in variants {
                            if let Type::Function(_, param_types, ret_ty) = self.get_type(ty)
                                && param_types.len() == arg_types.len()
                            {
                                let mut matches = true;
                                for (pt, at) in param_types.iter().zip(arg_types.iter()) {
                                    if !self.is_assignable(*at, *pt) {
                                        matches = false;
                                        break;
                                    }
                                }
                                if matches {
                                    matched_variant =
                                        Some((mangled_name, ty, ret_ty));
                                    break;
                                }
                            }
                        }

                        if let Some((mangled_name, ty, ret_ty)) = matched_variant {
                            typed_callee = TypedExpr {
                                kind: TypedExprKind::Variable(mangled_name),
                                ty,
                                span: typed_callee.span,
                            };
                            ret_ty
                        } else {
                            self.error(
                                expr.span,
                                DiagnosticCode::TypeMismatch,
                                "No matching overload found for arguments.",
                            );
                            self.session.types.borrow_mut().intern(Type::Error)
                        }
                    }
                    Type::Class(class_name, class_type_params) => {
                        let mut constructor_ty = self.classes.get(&class_name).and_then(|props| {
                            props
                                .get(&self.session.interner.borrow_mut().intern("init"))
                                .cloned()
                        });

                        if constructor_ty.is_none()
                            && let Some(generic_stmt) =
                                self.generic_registry.get_class(class_name).cloned()
                            && let ast::StmtKind::Class {
                                type_params,
                                methods,
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
                            for method in methods {
                                if let ast::StmtKind::Func { name, params, .. } = &method.kind
                                    && self.session.interner.borrow().lookup(*name) == "init"
                                {
                                    let mut param_types = Vec::new();
                                    for (_, ty) in params {
                                        param_types.push(self.parse_type(ty, expr.span));
                                    }

                                    let void_ty =
                                        self.session.types.borrow_mut().intern(Type::Void);
                                    constructor_ty =
                                        Some(self.session.types.borrow_mut().intern(
                                            Type::Function(Vec::new(), param_types, void_ty),
                                        ));
                                }
                            }
                            self.env.pop_scope();
                        }

                        let mut resolved_type_args = Vec::new();

                        if let Some(Type::Function(_, param_types, _)) =
                            constructor_ty.map(|id| self.get_type(id))
                        {
                            if param_types.len() != arg_types.len() {
                                self.error(
                                    expr.span,
                                    DiagnosticCode::TypeMismatch,
                                    &format!(
                                        "Constructor expected {} arguments, found {}.",
                                        param_types.len(),
                                        arg_types.len()
                                    ),
                                )
                            } else {
                                // Basic Local Inference & Checking
                                if !class_type_params.is_empty() {
                                    if type_args.is_empty() {
                                        // Infer from arguments
                                        let mut inferred_map = std::collections::HashMap::new();
                                        for (expected, actual) in
                                            param_types.iter().zip(arg_types.iter())
                                        {
                                            self.infer_generics(
                                                *expected,
                                                *actual,
                                                &mut inferred_map,
                                            );
                                        }

                                        for tp in &class_type_params {
                                            if let Some(ty) = inferred_map.get(tp) {
                                                resolved_type_args.push(*ty);
                                            } else {
                                                self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot infer generic type '{}'. Please provide explicit type arguments.", self.session.interner.borrow().lookup(*tp)));
                                                resolved_type_args.push(
                                                    self.session
                                                        .types
                                                        .borrow_mut()
                                                        .intern(Type::Error),
                                                );
                                            }
                                        }
                                    } else {
                                        // Explicit arguments provided
                                        if type_args.len() != class_type_params.len() {
                                            self.error(
                                                expr.span,
                                                DiagnosticCode::TypeMismatch,
                                                &format!(
                                                    "Expected {} generic arguments, found {}.",
                                                    class_type_params.len(),
                                                    type_args.len()
                                                ),
                                            )
                                        }
                                        for arg_expr in type_args {
                                            resolved_type_args
                                                .push(self.parse_type(arg_expr, expr.span));
                                        }
                                    }
                                }

                                // Instantiate the generic class!
                                if !class_type_params.is_empty() {
                                    let mangled_name = self.instantiate_generic_class(
                                        class_name,
                                        &class_type_params,
                                        &resolved_type_args,
                                    );

                                    // Rewrite the callee to point to the mangled name!
                                    let new_callee =
                                        TypedExpr {
                                            kind: TypedExprKind::Variable(mangled_name),
                                            ty: self.session.types.borrow_mut().intern(
                                                Type::Class(mangled_name, Vec::new()),
                                            ),
                                            span: callee.span,
                                        };

                                    // We must update ty to be Instance(mangled_name) instead of GenericInstance.
                                    let new_ty = self
                                        .session
                                        .types
                                        .borrow_mut()
                                        .intern(Type::Instance(mangled_name));

                                    // But wait, we still need to check argument types correctly!
                                    // We can just proceed, because substitute generic parameters handles the check.

                                    let mut replacements = std::collections::HashMap::new();
                                    for (tp, resolved) in
                                        class_type_params.iter().zip(resolved_type_args.iter())
                                    {
                                        replacements.insert(*tp, *resolved);
                                    }

                                    for (i, (expected, actual)) in
                                        param_types.iter().zip(arg_types.iter()).enumerate()
                                    {
                                        let expected_sub =
                                            self.substitute_generics(*expected, &replacements);
                                        if !self.is_assignable(*actual, expected_sub) {
                                            self.error(
                                                arguments[i].span,
                                                DiagnosticCode::TypeMismatch,
                                                &format!(
                                                    "Expected type '{}' for argument, found '{}'.",
                                                    self.session.format_type(expected_sub),
                                                    self.session.format_type(*actual)
                                                ),
                                            );
                                        }
                                    }

                                    return TypedExpr {
                                        kind: TypedExprKind::Call {
                                            callee: self.alloc(new_callee),
                                            type_args: Vec::new(),
                                            arguments: typed_args,
                                        },
                                        ty: new_ty,
                                        span: expr.span,
                                    };
                                }

                                // Substitute generic parameters when checking constructor argument types
                                for (i, (expected, actual)) in
                                    param_types.iter().zip(arg_types.iter()).enumerate()
                                {
                                    let expected_sub = if class_type_params.is_empty() {
                                        *expected
                                    } else {
                                        let mut type_map = std::collections::HashMap::new();
                                        for (i, p) in class_type_params.iter().enumerate() {
                                            if i < resolved_type_args.len() {
                                                type_map.insert(
                                                    *p,
                                                    resolved_type_args[i],
                                                );
                                            }
                                        }
                                        self.substitute_generics(*expected, &type_map)
                                    };

                                    if !self.is_assignable(*actual, expected_sub)
                                        && expected_sub
                                            != self.session.types.borrow_mut().intern(Type::Any)
                                        && *actual
                                            != self.session.types.borrow_mut().intern(Type::Error)
                                    {
                                        self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Argument {} to constructor expects '{}', found '{}'.", i + 1, self.session.format_type(expected_sub), self.session.format_type(*actual)))
                                    }
                                }
                            }
                        } else if !arg_types.is_empty() {
                            self.error(
                                expr.span,
                                DiagnosticCode::TypeMismatch,
                                &format!(
                                    "Class '{}' has no 'init' method but arguments were provided.",
                                    self.session.interner.borrow().lookup(class_name)
                                ),
                            )
                        }

                        if !class_type_params.is_empty() {
                            let mangled_name = self.instantiate_generic_class(
                                class_name,
                                &class_type_params,
                                &resolved_type_args,
                            );

                            let new_callee = TypedExpr {
                                kind: TypedExprKind::Variable(mangled_name),
                                ty: self
                                    .session
                                    .types
                                    .borrow_mut()
                                    .intern(Type::Class(mangled_name, Vec::new())),
                                span: callee.span,
                            };

                            let new_ty = self
                                .session
                                .types
                                .borrow_mut()
                                .intern(Type::Instance(mangled_name));

                            return TypedExpr {
                                kind: TypedExprKind::Call {
                                    callee: self.alloc(new_callee),
                                    type_args: Vec::new(),
                                    arguments: typed_args,
                                },
                                ty: new_ty,
                                span: expr.span,
                            };
                        }

                        self.session
                            .types
                            .borrow_mut()
                            .intern(Type::Instance(class_name))
                    }
                    Type::Function(func_type_params, param_types, ret_ty) => {
                        let mut inferred_map = std::collections::HashMap::new();

                        if !func_type_params.is_empty() {
                            if type_args.is_empty() {
                                // Infer from arguments
                                for (expected, actual) in param_types.iter().zip(arg_types.iter()) {
                                    self.infer_generics(*expected, *actual, &mut inferred_map);
                                }

                                // Infer from expected return type (contextual bidirectional inference)
                                if let Some(expected_result) = expected_ty {
                                    self.infer_generics(ret_ty, expected_result, &mut inferred_map);
                                }

                                for tp in &func_type_params {
                                    if !inferred_map.contains_key(tp) {
                                        self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot infer generic type '{}'. Please provide explicit type arguments.", self.session.interner.borrow().lookup(*tp)));
                                        inferred_map.insert(
                                            *tp,
                                            self.session.types.borrow_mut().intern(Type::Error),
                                        );
                                    }
                                }
                            } else {
                                if type_args.len() != func_type_params.len() {
                                    self.error(
                                        expr.span,
                                        DiagnosticCode::TypeMismatch,
                                        &format!(
                                            "Expected {} generic arguments, found {}.",
                                            func_type_params.len(),
                                            type_args.len()
                                        ),
                                    )
                                }
                                for (i, arg_expr) in type_args.iter().enumerate() {
                                    let ty = self.parse_type(arg_expr, expr.span);
                                    if i < func_type_params.len() {
                                        inferred_map.insert(func_type_params[i], ty);
                                    }
                                }
                            }
                        }

                        if param_types.len() != arg_types.len() {
                            self.error(
                                expr.span,
                                DiagnosticCode::TypeMismatch,
                                &format!(
                                    "Expected {} arguments, found {}.",
                                    param_types.len(),
                                    arg_types.len()
                                ),
                            )
                        } else {
                            for (i, (expected, actual)) in
                                param_types.iter().zip(arg_types.iter()).enumerate()
                            {
                                let expected_sub = if func_type_params.is_empty() {
                                    *expected
                                } else {
                                    self.substitute_generics(*expected, &inferred_map)
                                };

                                if !self.is_assignable(*actual, expected_sub)
                                    && expected_sub
                                        != self.session.types.borrow_mut().intern(Type::Any)
                                    && *actual
                                        != self.session.types.borrow_mut().intern(Type::Error)
                                {
                                    self.error(
                                        expr.span,
                                        DiagnosticCode::TypeMismatch,
                                        &format!(
                                            "Argument {} expected type '{}', found '{}'.",
                                            i + 1,
                                            self.session.format_type(expected_sub),
                                            self.session.format_type(*actual)
                                        ),
                                    );
                                }
                            }
                        }

                        if func_type_params.is_empty() {
                            ret_ty
                        } else {
                            let mut resolved_type_args = Vec::new();
                            for tp in &func_type_params {
                                resolved_type_args.push(inferred_map.get(tp).copied().unwrap_or(
                                    self.session.types.borrow_mut().intern(Type::Error),
                                ));
                            }

                            if let TypedExprKind::Variable(func_name) = &typed_callee.kind {
                                let func_name_str = self
                                    .session
                                    .interner
                                    .borrow()
                                    .lookup(*func_name)
                                    .to_string();
                                let func_sym =
                                    self.session.interner.borrow_mut().intern(&func_name_str);
                                let mangled = self.instantiate_generic_function(
                                    func_sym,
                                    &func_type_params,
                                    &resolved_type_args,
                                );

                                let new_callee = TypedExpr {
                                    kind: TypedExprKind::Variable(mangled),
                                    ty: self.session.types.borrow_mut().intern(Type::BuiltinFunc), // Can be treated as builtin or regular function
                                    span: callee.span,
                                };

                                let new_ret_ty = self.substitute_generics(ret_ty, &inferred_map);

                                return TypedExpr {
                                    kind: TypedExprKind::Call {
                                        callee: self.alloc(new_callee),
                                        type_args: Vec::new(),
                                        arguments: typed_args,
                                    },
                                    ty: new_ret_ty,
                                    span: expr.span,
                                };
                            }

                            self.substitute_generics(ret_ty, &inferred_map)
                        }
                    }
                    Type::EnumVariantConstructor(
                        enum_name,
                        variant_name,
                        func_type_params,
                        param_types,
                        ret_ty,
                    ) => {
                        let mut inferred_map = std::collections::HashMap::new();

                        if !func_type_params.is_empty() {
                            if type_args.is_empty() {
                                // Infer from arguments
                                for (expected, actual) in param_types.iter().zip(arg_types.iter()) {
                                    self.infer_generics(*expected, *actual, &mut inferred_map);
                                }

                                // Infer from expected return type (contextual bidirectional inference)
                                if let Some(expected_result) = expected_ty {
                                    self.infer_generics(ret_ty, expected_result, &mut inferred_map);
                                }

                                for tp in &func_type_params {
                                    if !inferred_map.contains_key(tp) {
                                        self.error(expr.span, DiagnosticCode::TypeMismatch, &format!("Cannot infer generic type '{}'. Please provide explicit type arguments.", self.session.interner.borrow().lookup(*tp)));
                                        inferred_map.insert(
                                            *tp,
                                            self.session.types.borrow_mut().intern(Type::Error),
                                        );
                                    }
                                }
                            } else {
                                if type_args.len() != func_type_params.len() {
                                    self.error(
                                        expr.span,
                                        DiagnosticCode::TypeMismatch,
                                        &format!(
                                            "Expected {} generic arguments, found {}.",
                                            func_type_params.len(),
                                            type_args.len()
                                        ),
                                    )
                                }
                                for (i, arg_expr) in type_args.iter().enumerate() {
                                    let ty = self.parse_type(arg_expr, expr.span);
                                    if i < func_type_params.len() {
                                        inferred_map.insert(func_type_params[i], ty);
                                    }
                                }
                            }
                        }

                        if param_types.len() != arg_types.len() {
                            self.error(
                                expr.span,
                                DiagnosticCode::TypeMismatch,
                                &format!(
                                    "Expected {} arguments, found {}.",
                                    param_types.len(),
                                    arg_types.len()
                                ),
                            )
                        } else {
                            for (i, (expected, actual)) in
                                param_types.iter().zip(arg_types.iter()).enumerate()
                            {
                                let expected_sub = if func_type_params.is_empty() {
                                    *expected
                                } else {
                                    self.substitute_generics(*expected, &inferred_map)
                                };

                                if !self.is_assignable(*actual, expected_sub)
                                    && expected_sub
                                        != self.session.types.borrow_mut().intern(Type::Any)
                                    && *actual
                                        != self.session.types.borrow_mut().intern(Type::Error)
                                {
                                    self.error(
                                        expr.span,
                                        DiagnosticCode::TypeMismatch,
                                        &format!(
                                            "Argument {} expected type '{}', found '{}'.",
                                            i + 1,
                                            self.session.format_type(expected_sub),
                                            self.session.format_type(*actual)
                                        ),
                                    )
                                }
                            }
                        }

                        let new_ret_ty = if func_type_params.is_empty() {
                            ret_ty
                        } else {
                            self.substitute_generics(ret_ty, &inferred_map)
                        };

                        return TypedExpr {
                            kind: TypedExprKind::Call {
                                callee: self.alloc(TypedExpr {
                                    kind: TypedExprKind::EnumVariant {
                                        enum_name,
                                        variant_name,
                                    },
                                    ty: self.session.types.borrow_mut().intern(Type::BuiltinFunc),
                                    span: callee.span,
                                }),
                                type_args: Vec::new(),
                                arguments: typed_args,
                            },
                            ty: new_ret_ty,
                            span: expr.span,
                        };
                    }
                    Type::Error => self.session.types.borrow_mut().intern(Type::Error),
                    _ => {
                        self.error(
                            expr.span,
                            DiagnosticCode::TypeMismatch,
                            "Cannot call non-function type.",
                        );
                        self.session.types.borrow_mut().intern(Type::Error)
                    }
                };
                (
                    TypedExprKind::Call {
                        callee: self.alloc(typed_callee),
                        type_args: type_args.clone(),
                        arguments: typed_args,
                    },
                    ty,
                )
            }
            ExprKind::Unary(op, right) => {
                let typed_right = self.check_expr(right);
                if typed_right.ty == self.session.types.borrow_mut().intern(Type::Error) {
                    return TypedExpr::new(
                        TypedExprKind::Unary(op.clone(), self.alloc(typed_right)),
                        self.session.types.borrow_mut().intern(Type::Error),
                        expr.span,
                    );
                }

                let ty = match op {
                    UnaryOp::Negate => {
                        if typed_right.ty == self.session.types.borrow_mut().intern(Type::Int)
                            || typed_right.ty == self.session.types.borrow_mut().intern(Type::Float)
                        {
                            typed_right.ty
                        } else {
                            self.error(
                                expr.span,
                                DiagnosticCode::TypeMismatch,
                                &format!(
                                    "Cannot negate type '{}'.",
                                    self.session.format_type(typed_right.ty)
                                ),
                            );
                            self.session.types.borrow_mut().intern(Type::Error)
                        }
                    }
                };
                (
                    TypedExprKind::Unary(op.clone(), self.alloc(typed_right)),
                    ty,
                )
            }
            ExprKind::Range { start, end } => {
                let int_ty = self.session.types.borrow_mut().intern(Type::Int);
                let typed_start = self.check_expr_with_expected(start, Some(int_ty));
                let typed_end = self.check_expr_with_expected(end, Some(int_ty));

                if typed_start.ty != self.session.types.borrow_mut().intern(Type::Int)
                    || typed_end.ty != self.session.types.borrow_mut().intern(Type::Int)
                {
                    self.error(
                        expr.span,
                        DiagnosticCode::TypeMismatch,
                        "Range bounds must be integers.",
                    );
                }

                (
                    TypedExprKind::Range {
                        start: self.alloc(typed_start),
                        end: self.alloc(typed_end),
                    },
                    self.session.types.borrow_mut().intern(Type::Range),
                )
            }
            ExprKind::Binary(left, op, right) => {
                let typed_left = self.check_expr(left);
                let typed_right = self.check_expr(right);

                if typed_left.ty == self.session.types.borrow_mut().intern(Type::Error)
                    || typed_right.ty == self.session.types.borrow_mut().intern(Type::Error)
                {
                    return TypedExpr::new(
                        TypedExprKind::Binary(
                            self.alloc(typed_left),
                            op.clone(),
                            self.alloc(typed_right),
                        ),
                        self.session.types.borrow_mut().intern(Type::Error),
                        expr.span,
                    );
                }

                let ty = match op {
                    BinaryOp::Add | BinaryOp::Subtract | BinaryOp::Multiply | BinaryOp::Divide => {
                        let int_ty = self.session.types.borrow_mut().intern(Type::Int);
                        if (self.is_assignable(typed_left.ty, int_ty)
                            || typed_left.ty == self.session.types.borrow_mut().intern(Type::Float))
                            && self.is_assignable(typed_left.ty, typed_right.ty)
                        {
                            typed_left.ty
                        } else if *op == BinaryOp::Add
                            && typed_left.ty == self.session.types.borrow_mut().intern(Type::String)
                            && typed_right.ty
                                == self.session.types.borrow_mut().intern(Type::String)
                        {
                            self.session.types.borrow_mut().intern(Type::String)
                        } else {
                            self.error(
                                expr.span,
                                DiagnosticCode::TypeMismatch,
                                &format!(
                                    "Cannot apply operator to types '{}' and '{}'.",
                                    self.session.format_type(typed_left.ty),
                                    self.session.format_type(typed_right.ty)
                                ),
                            );
                            self.session.types.borrow_mut().intern(Type::Error)
                        }
                    }
                    BinaryOp::Equal | BinaryOp::NotEqual => {
                        if !self.is_assignable(typed_left.ty, typed_right.ty)
                            && !self.is_assignable(typed_right.ty, typed_left.ty)
                        {
                            self.error(
                                expr.span,
                                DiagnosticCode::TypeMismatch,
                                &format!(
                                    "Cannot compare types '{}' and '{}' for equality.",
                                    self.session.format_type(typed_left.ty),
                                    self.session.format_type(typed_right.ty)
                                ),
                            );
                            self.session.types.borrow_mut().intern(Type::Error)
                        } else {
                            self.session.types.borrow_mut().intern(Type::Boolean)
                        }
                    }
                    BinaryOp::Less
                    | BinaryOp::LessEqual
                    | BinaryOp::Greater
                    | BinaryOp::GreaterEqual => {
                        if !self.is_assignable(typed_left.ty, typed_right.ty)
                            && !self.is_assignable(typed_right.ty, typed_left.ty)
                        {
                            self.error(
                                expr.span,
                                DiagnosticCode::TypeMismatch,
                                &format!(
                                    "Cannot apply comparison to types '{}' and '{}'.",
                                    self.session.format_type(typed_left.ty),
                                    self.session.format_type(typed_right.ty)
                                ),
                            );
                        }
                        self.session.types.borrow_mut().intern(Type::Boolean)
                    }
                };
                (
                    TypedExprKind::Binary(
                        self.alloc(typed_left),
                        op.clone(),
                        self.alloc(typed_right),
                    ),
                    ty,
                )
            }
        };
        TypedExpr::new(kind, ty, expr.span)
    }
}
