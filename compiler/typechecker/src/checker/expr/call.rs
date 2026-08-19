use super::super::*;
use session::types::{Type, TypeId};

impl<'a> TypeChecker<'a> {
    pub(crate) fn check_call_expr(
        &mut self,
        callee: &Expr<'a>,
        type_args: &[TypeExpr<'a>],
        arguments: &[Expr<'a>],
        expected_ty: Option<TypeId>,
        span: Span,
    ) -> (TypedExprKind<'a>, TypeId) {
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
            Type::Class(class_name, _) | Type::Struct(class_name, _) => {
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
                            matched_variant = Some((mangled_name, ty, ret_ty));
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
                        span,
                        DiagnosticCode::TypeMismatch,
                        "No matching overload found for arguments.",
                    );
                    self.session.types.borrow_mut().intern(Type::Error)
                }
            }
            Type::Class(class_name, class_type_params)
            | Type::Struct(class_name, class_type_params) => {
                let mut constructor_ty = self.classes.get(&class_name).and_then(|props| {
                    props
                        .get(&self.session.interner.borrow_mut().intern("init"))
                        .cloned()
                });

                if constructor_ty.is_none()
                    && let Some(generic_stmt) = self.generic_registry.get_class(class_name).cloned()
                    && let StmtKind::Class {
                        type_params,
                        methods,
                        ..
                    }
                    | StmtKind::Struct {
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
                        if let StmtKind::Func { name, params, .. } = &method.kind
                            && self.session.interner.borrow().lookup(*name) == "init"
                        {
                            let mut param_types = Vec::new();
                            for (_, ty) in params {
                                param_types.push(self.parse_type(ty, span));
                            }

                            let void_ty = self.session.types.borrow_mut().intern(Type::Void);
                            constructor_ty =
                                Some(self.session.types.borrow_mut().intern(Type::Function(
                                    Vec::new(),
                                    param_types,
                                    void_ty,
                                )));
                        }
                    }
                    self.env.pop_scope();
                }

                let mut resolved_type_args = Vec::new();

                if !class_type_params.is_empty() {
                    if type_args.is_empty() {
                        if let Some(Type::Function(_, param_types, _)) =
                            constructor_ty.map(|id| self.get_type(id))
                        {
                            // Infer from arguments
                            let mut inferred_map = std::collections::HashMap::new();
                            for (expected, actual) in param_types.iter().zip(arg_types.iter()) {
                                self.infer_generics(*expected, *actual, &mut inferred_map);
                            }

                            for tp in &class_type_params {
                                if let Some(ty) = inferred_map.get(tp) {
                                    resolved_type_args.push(*ty);
                                } else {
                                    self.error(span, DiagnosticCode::TypeMismatch, &format!("Cannot infer generic type '{}'. Please provide explicit type arguments.", self.session.interner.borrow().lookup(*tp)));
                                    resolved_type_args
                                        .push(self.session.types.borrow_mut().intern(Type::Error));
                                }
                            }
                        } else {
                            self.error(span, DiagnosticCode::TypeMismatch, "Cannot infer generic type. Please provide explicit type arguments.");
                        }
                    } else {
                        // Explicit arguments provided
                        if type_args.len() != class_type_params.len() {
                            self.error(
                                span,
                                DiagnosticCode::TypeMismatch,
                                &format!(
                                    "Expected {} generic arguments, found {}.",
                                    class_type_params.len(),
                                    type_args.len()
                                ),
                            )
                        }
                        for arg_expr in type_args {
                            resolved_type_args.push(self.parse_type(arg_expr, span));
                        }
                    }
                }

                if let Some(Type::Function(_, param_types, _)) =
                    constructor_ty.map(|id| self.get_type(id))
                {
                    if param_types.len() != arg_types.len() {
                        self.error(
                            span,
                            DiagnosticCode::TypeMismatch,
                            &format!(
                                "Constructor expected {} arguments, found {}.",
                                param_types.len(),
                                arg_types.len()
                            ),
                        )
                    } else {
                        // Instantiate the generic class!
                        if !class_type_params.is_empty() {
                            let mut is_concrete = true;
                            for ty in &resolved_type_args {
                                if let Type::Generic(_) = self.get_type(*ty) {
                                    is_concrete = false;
                                    break;
                                }
                            }

                            let (new_callee, new_ty) =
                                if is_concrete {
                                    let mangled_name = self.instantiate_generic_class(
                                        class_name,
                                        &class_type_params,
                                        &resolved_type_args,
                                    );

                                    // Rewrite the callee to point to the mangled name!
                                    let new_callee = TypedExpr {
                                        kind: TypedExprKind::Variable(mangled_name),
                                        ty: self
                                            .session
                                            .types
                                            .borrow_mut()
                                            .intern(Type::Class(mangled_name, Vec::new())),
                                        span: callee.span,
                                    };

                                    // We must update ty to be Instance(mangled_name) instead of GenericInstance.
                                    let new_ty = self
                                        .session
                                        .types
                                        .borrow_mut()
                                        .intern(Type::Instance(mangled_name));
                                    (new_callee, new_ty)
                                } else {
                                    let new_ty = self.session.types.borrow_mut().intern(
                                        Type::GenericInstance(
                                            class_name,
                                            resolved_type_args.clone(),
                                        ),
                                    );
                                    (typed_callee.clone(), new_ty)
                                };

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

                            return (
                                TypedExprKind::Call {
                                    callee: self.alloc(new_callee),
                                    type_args: Vec::new(),
                                    arguments: typed_args,
                                },
                                new_ty,
                            );
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
                                        type_map.insert(*p, resolved_type_args[i]);
                                    }
                                }
                                self.substitute_generics(*expected, &type_map)
                            };

                            if !self.is_assignable(*actual, expected_sub)
                                && expected_sub != self.session.types.borrow_mut().intern(Type::Any)
                                && *actual != self.session.types.borrow_mut().intern(Type::Error)
                            {
                                self.error(
                                    span,
                                    DiagnosticCode::TypeMismatch,
                                    &format!(
                                        "Argument {} to constructor expects '{}', found '{}'.",
                                        i + 1,
                                        self.session.format_type(expected_sub),
                                        self.session.format_type(*actual)
                                    ),
                                )
                            }
                        }
                    }
                } else if !arg_types.is_empty() {
                    self.error(
                        span,
                        DiagnosticCode::TypeMismatch,
                        &format!(
                            "Class '{}' has no 'init' method but arguments were provided.",
                            self.session.interner.borrow().lookup(class_name)
                        ),
                    )
                }

                if !class_type_params.is_empty() {
                    let mut is_concrete = true;
                    for ty in &resolved_type_args {
                        if let Type::Generic(_) = self.get_type(*ty) {
                            is_concrete = false;
                            break;
                        }
                    }

                    let (new_callee, new_ty) = if is_concrete {
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
                        (new_callee, new_ty)
                    } else {
                        let new_ty = self
                            .session
                            .types
                            .borrow_mut()
                            .intern(Type::GenericInstance(
                                class_name,
                                resolved_type_args.clone(),
                            ));
                        (typed_callee.clone(), new_ty)
                    };

                    return (
                        TypedExprKind::Call {
                            callee: self.alloc(new_callee),
                            type_args: Vec::new(),
                            arguments: typed_args,
                        },
                        new_ty,
                    );
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
                                self.error(span, DiagnosticCode::TypeMismatch, &format!("Cannot infer generic type '{}'. Please provide explicit type arguments.", self.session.interner.borrow().lookup(*tp)));
                                inferred_map.insert(
                                    *tp,
                                    self.session.types.borrow_mut().intern(Type::Error),
                                );
                            }
                        }
                    } else {
                        if type_args.len() > func_type_params.len() {
                            self.error(
                                span,
                                DiagnosticCode::TypeMismatch,
                                &format!(
                                    "Expected at most {} generic arguments, found {}.",
                                    func_type_params.len(),
                                    type_args.len()
                                ),
                            )
                        }
                        for (i, arg_expr) in type_args.iter().enumerate() {
                            let ty = self.parse_type(arg_expr, span);
                            if i < func_type_params.len() {
                                inferred_map.insert(func_type_params[i], ty);
                            }
                        }
                        if type_args.len() < func_type_params.len() {
                            for (expected, actual) in param_types.iter().zip(arg_types.iter()) {
                                self.infer_generics(*expected, *actual, &mut inferred_map);
                            }
                            if let Some(expected_result) = expected_ty {
                                self.infer_generics(ret_ty, expected_result, &mut inferred_map);
                            }
                        }
                        for tp in &func_type_params {
                            if !inferred_map.contains_key(tp) {
                                self.error(span, DiagnosticCode::TypeMismatch, &format!("Cannot infer generic type '{}'. Please provide explicit type arguments.", self.session.interner.borrow().lookup(*tp)));
                                inferred_map.insert(
                                    *tp,
                                    self.session.types.borrow_mut().intern(Type::Error),
                                );
                            }
                        }
                    }
                }

                if param_types.len() != arg_types.len() {
                    self.error(
                        span,
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
                            && expected_sub != self.session.types.borrow_mut().intern(Type::Any)
                            && *actual != self.session.types.borrow_mut().intern(Type::Error)
                        {
                            self.error(
                                span,
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
                        resolved_type_args.push(
                            inferred_map
                                .get(tp)
                                .copied()
                                .unwrap_or(self.session.types.borrow_mut().intern(Type::Error)),
                        );
                    }

                    if let TypedExprKind::Variable(func_name) = &typed_callee.kind {
                        let func_name_str = self
                            .session
                            .interner
                            .borrow()
                            .lookup(*func_name)
                            .to_string();
                        let func_sym = self.session.interner.borrow_mut().intern(&func_name_str);
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

                        return (
                            TypedExprKind::Call {
                                callee: self.alloc(new_callee),
                                type_args: Vec::new(),
                                arguments: typed_args,
                            },
                            new_ret_ty,
                        );
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
                                self.error(span, DiagnosticCode::TypeMismatch, &format!("Cannot infer generic type '{}'. Please provide explicit type arguments.", self.session.interner.borrow().lookup(*tp)));
                                inferred_map.insert(
                                    *tp,
                                    self.session.types.borrow_mut().intern(Type::Error),
                                );
                            }
                        }
                    } else {
                        if type_args.len() != func_type_params.len() {
                            self.error(
                                span,
                                DiagnosticCode::TypeMismatch,
                                &format!(
                                    "Expected {} generic arguments, found {}.",
                                    func_type_params.len(),
                                    type_args.len()
                                ),
                            )
                        }
                        for (i, arg_expr) in type_args.iter().enumerate() {
                            let ty = self.parse_type(arg_expr, span);
                            if i < func_type_params.len() {
                                inferred_map.insert(func_type_params[i], ty);
                            }
                        }
                    }
                }

                if param_types.len() != arg_types.len() {
                    self.error(
                        span,
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
                            && expected_sub != self.session.types.borrow_mut().intern(Type::Any)
                            && *actual != self.session.types.borrow_mut().intern(Type::Error)
                        {
                            self.error(
                                span,
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

                if !func_type_params.is_empty() {
                    let resolved_type_args: Vec<_> = func_type_params
                        .iter()
                        .map(|p| {
                            *inferred_map
                                .get(p)
                                .unwrap_or(&self.session.types.borrow_mut().intern(Type::Error))
                        })
                        .collect();
                    let mut is_concrete = true;
                    for ty in &resolved_type_args {
                        if let Type::Generic(_) = self.get_type(*ty) {
                            is_concrete = false;
                            break;
                        }
                    }

                    if is_concrete {
                        let mangled_name = self.instantiate_generic_class(
                            enum_name,
                            &func_type_params,
                            &resolved_type_args,
                        );

                        let new_ret_ty = self
                            .session
                            .types
                            .borrow_mut()
                            .intern(Type::Enum(mangled_name, Vec::new()));
                        return (
                            TypedExprKind::Call {
                                callee: self.alloc(TypedExpr {
                                    kind: TypedExprKind::EnumVariant {
                                        enum_name: mangled_name,
                                        variant_name,
                                    },
                                    ty: self.session.types.borrow_mut().intern(Type::BuiltinFunc),
                                    span: callee.span,
                                }),
                                type_args: Vec::new(),
                                arguments: typed_args,
                            },
                            new_ret_ty,
                        );
                    } else {
                        let new_ret_ty = self
                            .session
                            .types
                            .borrow_mut()
                            .intern(Type::GenericInstance(enum_name, resolved_type_args.clone()));
                        return (
                            TypedExprKind::Call {
                                callee: self.alloc(typed_callee),
                                type_args: Vec::new(),
                                arguments: typed_args,
                            },
                            new_ret_ty,
                        );
                    }
                }

                let new_ret_ty = if func_type_params.is_empty() {
                    ret_ty
                } else {
                    self.substitute_generics(ret_ty, &inferred_map)
                };

                return (
                    TypedExprKind::Call {
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
                    new_ret_ty,
                );
            }
            Type::Error => self.session.types.borrow_mut().intern(Type::Error),
            _ => {
                self.error(
                    span,
                    DiagnosticCode::TypeMismatch,
                    "Cannot call non-function type.",
                );
                self.session.types.borrow_mut().intern(Type::Error)
            }
        };
        (
            TypedExprKind::Call {
                callee: self.alloc(typed_callee),
                type_args: type_args.to_vec(),
                arguments: typed_args,
            },
            ty,
        )
    }
}
