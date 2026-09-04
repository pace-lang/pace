use super::TypeChecker;
use crate::env::Type;
use pace_ast::{BinaryOp, Visibility};
use pace_hir::Expr;
use pace_errors::TypeError;

impl<'a> TypeChecker<'a> {
    pub(crate) fn check_expr_closure(
        &mut self,
        params: &[(ustr::Ustr, pace_ast::TypeAnnotation)],
        return_type: Option<&pace_ast::TypeAnnotation>,
        body: pace_hir::ExprId,
    ) -> Type {
        self.env.push_scope();

        let mut param_types = Vec::new();
        for (param_name, param_ty_ann) in params {
            let param_ty = self.resolve_type_name(param_ty_ann);
            param_types.push(param_ty.clone());
            let _ = self
                .env
                .define(*param_name, param_ty, pace_span::Span::default(), true);
        }

        let ret_ty = if let Some(rt) = return_type {
            self.resolve_type_name(rt)
        } else {
            Type::Unknown
        };

        let old_expected_return = self.current_return_type.clone();
        self.current_return_type = Some(ret_ty.clone());

        let body_ty = self.check_expr(body);

        self.current_return_type = old_expected_return;
        self.pop_scope_and_check_unused();

        let final_ret = if ret_ty != Type::Unknown {
            ret_ty
        } else {
            body_ty
        };

        Type::Function {
            generic_params: None,
            params: param_types,
            return_type: Box::new(final_ret),
        }
    }

    pub(crate) fn check_expr_identifier(&mut self, name: &ustr::Ustr) -> Type {
        if let Some(var_info) = self.env.get_mut(*name) {
            var_info.is_used = true;
        }
        match self.env.get(*name) {
            Some(ty) => ty.clone(),
            None => {
                // Check if it's a class/struct for instantiation
                // Check if it's a module item
                if self.env.classes.contains_key(name) {
                    Type::Class(*name)
                } else if self.env.actors.contains_key(name) {
                    Type::Actor(*name)
                } else if self.env.structs.contains_key(name) {
                    Type::Struct(*name)
                } else if self.env.enums.contains_key(name) {
                    Type::Enum(*name)
                } else if let Some(global) = self.env.global_vars.get(name) {
                    global.ty.clone()
                } else if let Some(sig) = self.env.functions.get(name) {
                    Type::Function {
                        generic_params: sig.generic_params.clone(),
                        params: sig.params.clone(),
                        return_type: Box::new(sig.return_type.clone()),
                    }
                } else {
                    let suggestion = self.env.find_closest_variable(*name);
                    let help_text = if let Some(sug) = suggestion {
                        format!("Did you mean '{}'?", sug)
                    } else {
                        "Variable does not exist.".into()
                    };
                    self.errors.push(TypeError::UnknownIdentifier {
                        name: name.to_string(),
                        help_text,
                        src: self.get_source(),
                        span: self.get_span_for(name),
                    });
                    Type::Error
                }
            }
        }
    }

    pub(crate) fn check_expr(&mut self, expr_id: pace_hir::ExprId) -> Type {
        let ty = self.check_expr_inner(expr_id);
        self.env.node_types.insert(expr_id, ty.clone());
        ty
    }

    pub(crate) fn check_expr_inner(&mut self, expr_id: pace_hir::ExprId) -> Type {
        let expr = self.arena.get_expr(expr_id);
        match expr {
            Expr::IntLiteral(_) => Type::Int,
            Expr::FloatLiteral(_) => Type::Float,
            Expr::StringLiteral(_) => Type::String,
            Expr::GenericInstantiation {
                callee,
                generic_args,
            } => {
                let base_ty = self.check_expr(*callee);
                let mut arg_types = Vec::new();
                for arg in generic_args {
                    arg_types.push(self.resolve_type_name(arg));
                }
                Type::GenericInstance {
                    base: Box::new(base_ty),
                    args: arg_types,
                }
            }
            Expr::InterpolatedString(parts) => {
                for part in parts {
                    let ty = self.check_expr(*part);
                    if ty != Type::String
                        && ty != Type::Int
                        && ty != Type::Float
                        && ty != Type::Bool
                    {
                        {
                            self.errors.push(TypeError::Generic {
                                src: self.get_source(),
                                span: self.current_span,
                                message: format!("Cannot interpolate value of type {:?}", ty),
                            });
                            return Type::Error;
                        };
                    }
                }
                Type::String
            }
            Expr::BoolLiteral(_) => Type::Bool,
            Expr::Null => Type::Null,
            Expr::Closure {
                params,
                return_type,
                body,
            } => self.check_expr_closure(params, return_type.as_ref(), *body),
            Expr::Block(stmts) => {
                self.env.push_scope();
                for stmt_id in stmts {
                    self.check_stmt(*stmt_id);
                }
                self.pop_scope_and_check_unused();
                Type::Void
            }
            Expr::Identifier(name) => {
                let ty = self.check_expr_identifier(name);
                if let Some(def_span) = self.env.get_definition_span(*name) {
                    self.env.node_definitions.insert(expr_id, def_span);
                }
                ty
            },
            Expr::Unary {
                op,
                expr: inner_expr,
            } => {
                let inner_ty = self.check_expr(*inner_expr);
                match op {
                    pace_ast::UnaryOp::Not => {
                        if inner_ty != Type::Bool
                            && inner_ty != Type::Unknown
                            && inner_ty != Type::Error
                        {
                            self.errors.push(pace_errors::TypeError::Generic {
                                message: format!(
                                    "Type mismatch: expected Bool, found {:?}",
                                    inner_ty
                                ),
                                src: self.get_source(),
                                span: self.current_span,
                            });
                        }
                        Type::Bool
                    }
                    pace_ast::UnaryOp::Neg | pace_ast::UnaryOp::BitNot => {
                        if inner_ty != Type::Int
                            && inner_ty != Type::Float
                            && inner_ty != Type::Unknown
                            && inner_ty != Type::Error
                        {
                            self.errors.push(pace_errors::TypeError::Generic {
                                message: format!(
                                    "Type mismatch: expected numeric type, found {:?}",
                                    inner_ty
                                ),
                                src: self.get_source(),
                                span: self.current_span,
                            });
                        }
                        inner_ty
                    }
                }
            }
            Expr::Binary { left, op, right } => {
                let left_ty = self.check_expr(*left);
                let right_ty = self.check_expr(*right);

                let mut types_match = left_ty == right_ty;
                if matches!(left_ty, Type::Nullable(_)) && right_ty == Type::Null {
                    types_match = true;
                }
                if matches!(right_ty, Type::Nullable(_)) && left_ty == Type::Null {
                    types_match = true;
                }

                if matches!(op, pace_ast::BinaryOp::Add) && (left_ty == Type::String || right_ty == Type::String) {
                    let other_ty = if left_ty == Type::String { &right_ty } else { &left_ty };
                    if !matches!(other_ty, Type::String | Type::Int | Type::Float | Type::Bool | Type::Unknown | Type::Any) {
                        self.errors.push(TypeError::Generic {
                            src: self.get_source(),
                            span: self.current_span,
                            message: format!("Cannot concatenate String with {:?}", other_ty),
                        });
                        return Type::Error;
                    }
                    types_match = true;
                }

                if !types_match
                    && left_ty != Type::Unknown
                    && right_ty != Type::Unknown
                    && left_ty != Type::Any
                    && right_ty != Type::Any
                {
                    {
                        self.errors.push(TypeError::Generic {
                            src: self.get_source(),
                            span: self.current_span,
                            message: format!(
                                "Type mismatch in binary operation: {:?} and {:?}",
                                left_ty, right_ty
                            ),
                        });
                        return Type::Error;
                    };
                }

                match op {
                    BinaryOp::Add => {
                        if left_ty == Type::String || right_ty == Type::String {
                            Type::String
                        } else if left_ty == Type::Int
                            || left_ty == Type::Float
                            || left_ty == Type::Unknown
                            || left_ty == Type::Any
                            || right_ty == Type::Unknown
                            || right_ty == Type::Any
                        {
                            left_ty
                        } else {
                            {
                                self.errors.push(TypeError::Generic {
                                    src: self.get_source(),
                                    span: self.current_span,
                                    message: "Arithmetic operations require numeric types"
                                        .to_string(),
                                });
                                Type::Error
                            }
                        }
                    }
                    BinaryOp::Sub
                    | BinaryOp::Mul
                    | BinaryOp::Div
                    | BinaryOp::Mod => {
                        if left_ty == Type::Int
                            || left_ty == Type::Float
                            || left_ty == Type::Unknown
                            || left_ty == Type::Any
                            || right_ty == Type::Unknown
                            || right_ty == Type::Any
                        {
                            left_ty
                        } else {
                            {
                                self.errors.push(TypeError::Generic {
                                    src: self.get_source(),
                                    span: self.current_span,
                                    message: "Arithmetic operations require numeric types"
                                        .to_string(),
                                });
                                Type::Error
                            }
                        }
                    }
                    BinaryOp::Eq | BinaryOp::NotEq => Type::Bool,
                    BinaryOp::Less | BinaryOp::LessEq | BinaryOp::Greater | BinaryOp::GreaterEq => {
                        if left_ty == Type::Int || left_ty == Type::Float {
                            Type::Bool
                        } else {
                            {
                                self.errors.push(TypeError::Generic {
                                    src: self.get_source(),
                                    span: self.current_span,
                                    message: "Relational operations require numeric types"
                                        .to_string(),
                                });
                                Type::Error
                            }
                        }
                    }
                    BinaryOp::And | BinaryOp::Or => {
                        if left_ty == Type::Bool {
                            Type::Bool
                        } else {
                            {
                                self.errors.push(TypeError::Generic {
                                    src: self.get_source(),
                                    span: self.current_span,
                                    message: "Logical operations require boolean types".into(),
                                });
                                Type::Error
                            }
                        }
                    }
                }
            }
            Expr::Assign { target, value } => {
                let val_ty = self.check_expr(*value);

                if let Expr::Identifier(name) = self.arena.get_expr(*target) {
                    let mut is_err = false;
                    let mut err_msg = String::new();
                    let mut var_span = pace_span::Span::default();

                    if let Some(var_info) = self.env.get_var_info(*name).cloned() {
                        if !var_info.is_mutable {
                            is_err = true;
                            err_msg = format!("Cannot assign to immutable variable '{}'", name);
                            var_span = var_info.span;
                        } else if !self.is_assignable_to(&val_ty, &var_info.ty)
                        {
                            is_err = true;
                            err_msg = format!(
                                "Type mismatch: cannot assign {:?} to variable of type {:?}",
                                val_ty, var_info.ty
                            );
                            var_span = var_info.span;
                        } else {
                            if let Some(v) = self.env.get_mut(*name) { v.is_used = true; }
                        }
                    } else if let Some(global) = self.env.global_vars.get(name).cloned() {
                        if !global.is_mutable {
                            is_err = true;
                            err_msg =
                                format!("Cannot assign to immutable global variable '{}'", name);
                            var_span = global.span;
                        } else if !self.is_assignable_to(&val_ty, &global.ty)
                        {
                            is_err = true;
                            err_msg = format!(
                                "Type mismatch: cannot assign {:?} to global variable of type {:?}",
                                val_ty, global.ty
                            );
                            var_span = global.span;
                        }
                    } else {
                        let suggestion = self.env.find_closest_variable(*name);
                        let help_text = if let Some(sug) = suggestion {
                            format!("Did you mean '{}'?", sug)
                        } else {
                            "Variable does not exist.".into()
                        };
                        self.errors.push(TypeError::UnknownIdentifier {
                            name: name.to_string(),
                            help_text,
                            src: self.get_source(),
                            span: self.get_span_for(name),
                        });
                        return Type::Error;
                    }

                    if is_err {
                        self.errors.push(TypeError::Generic {
                            src: self.get_source(),
                            span: var_span,
                            message: err_msg,
                        });
                        return Type::Error;
                    }
                    val_ty
                } else if let Expr::MemberAccess {
                    object,
                    property: _,
                    computed_class: _,
                    is_static_operator: _,
                } = self.arena.get_expr(*target)
                {
                    let _obj_ty = self.check_expr(*object);
                    // Simple validation for now - real validation needs class layout check
                    val_ty
                } else {
                    {
                        self.errors.push(TypeError::Generic {
                            src: self.get_source(),
                            span: self.current_span,
                            message: "Invalid assignment target".into(),
                        });
                        Type::Error
                    }
                }
            }
            Expr::Call { callee, args } => {
                let callee_ty = self.check_expr(*callee);

                let mut arg_types = Vec::new();
                for arg in args {
                    arg_types.push(self.check_expr(*arg));
                }

                // If callee is a known class/struct, it's a constructor call
                if let Type::Class(name) = &callee_ty {
                    if let Some(_sig) = self.env.classes.get(name) {
                        return Type::Class(*name);
                    }
                } else if let Type::Actor(name) = &callee_ty
                    && let Some(_sig) = self.env.actors.get(name)
                {
                    return Type::Actor(*name);
                } else if let Type::Struct(name) = &callee_ty
                    && let Some(_sig) = self.env.structs.get(name)
                {
                    return Type::Struct(*name);
                } else if let Type::Enum(name) = &callee_ty
                    && let Some(_sig) = self.env.enums.get(name)
                {
                    return Type::Enum(*name);
                }

                // If it's a function or method, we need its signature
                // Currently, callee_ty might just be Type::Unknown if it was a MemberAccess
                // So if we don't know the type, we just return Unknown.

                let mut actual_callee_ty = callee_ty.clone();
                if let Type::GenericInstance {
                    base,
                    args: gen_args,
                } = &callee_ty
                    && let Type::Function {
                        generic_params: Some(g_params),
                        params,
                        return_type,
                    } = &**base
                    {
                        let mut substs = std::collections::HashMap::new();
                        if g_params.len() == gen_args.len() {
                            for (p, arg) in g_params.iter().zip(gen_args.iter()) {
                                substs.insert(p.name, arg.clone());
                            }
                            actual_callee_ty = Type::Function {
                                generic_params: None,
                                params: params
                                    .iter()
                                    .map(|p| p.resolve_generics(&substs))
                                    .collect(),
                                return_type: Box::new(return_type.resolve_generics(&substs)),
                            };
                        } else {
                            self.errors.push(TypeError::Generic {
                                src: self.get_source(),
                                span: self.current_span,
                                message: format!(
                                    "Generic function expects {} type arguments, got {}",
                                    g_params.len(),
                                    gen_args.len()
                                ),
                            });
                            return Type::Error;
                        }
                    }

                // For first-class function values (closures, callbacks)
                if let Type::Function {
                    generic_params: _,
                    params,
                    return_type,
                } = &actual_callee_ty
                {
                    if params.len() != args.len() {
                        {
                            self.errors.push(TypeError::Generic {
                                src: self.get_source(),
                                span: self.current_span,
                                message: format!(
                                    "Function expects {} arguments, got {}",
                                    params.len(),
                                    args.len()
                                ),
                            });
                            return Type::Error;
                        };
                    }

                    for (i, arg_ty) in arg_types.iter().enumerate() {
                        let expected_ty = &params[i];
                        if expected_ty != &Type::Any && expected_ty != arg_ty && arg_ty != &Type::Unknown {
                                self.errors.push(TypeError::Generic {
                                    src: self.get_source(),
                                    span: self.current_span,
                                    message: format!(
                                        "Type mismatch in argument {}: expected {:?}, got {:?}",
                                        i + 1,
                                        expected_ty,
                                        arg_ty
                                    ),
                                });
                                return Type::Error;
                            }
                    }
                    return (**return_type).clone();
                }

                // For direct global function calls
                if let Expr::Identifier(func_name) = self.arena.get_expr(*callee)
                    && let Some(sig) = self.env.functions.get(&ustr::Ustr::from(func_name))
                {
                    if sig.visibility == Visibility::Private && sig.module != self.current_module {
                        {
                            self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                                message: format!("Function '{}' is private and cannot be accessed outside of module '{}'", func_name, sig.module)
                            });
                            return Type::Error;
                        };
                    }
                    if sig.params.len() != args.len() {
                        {
                            self.errors.push(TypeError::Generic {
                                src: self.get_source(),
                                span: self.current_span,
                                message: format!(
                                    "Function '{}' expects {} arguments, got {}",
                                    func_name,
                                    sig.params.len(),
                                    args.len()
                                ),
                            });
                            return Type::Error;
                        };
                    }

                    for (i, arg_ty) in arg_types.iter().enumerate() {
                        let expected_ty = &sig.params[i];
                        if expected_ty != &Type::Any && expected_ty != arg_ty {
                            {
                                self.errors.push(TypeError::Generic {
                                    src: self.get_source(),
                                    span: self.current_span,
                                    message: format!(
                                        "Type mismatch in argument {}: expected {:?}, got {:?}",
                                        i + 1,
                                        expected_ty,
                                        arg_ty
                                    ),
                                });
                                return Type::Error;
                            };
                        }
                    }
                    return sig.return_type.clone();
                }

                // For member access calls (e.g. self.client.get())
                // MemberAccess returns the method's return type, so we just return callee_ty
                callee_ty
            }
            Expr::MemberAccess {
                object,
                property,
                computed_class: _,
                is_static_operator,
            } => {
                let mut is_namespace_access = false;
                let mut base_ident = None;
                if let Expr::Identifier(name) = self.arena.get_expr(*object) {
                    base_ident = Some(*name);
                } else if let Expr::GenericInstantiation { callee, .. } =
                    self.arena.get_expr(*object)
                    && let Expr::Identifier(name) = self.arena.get_expr(*callee)
                {
                    base_ident = Some(*name);
                }
                if let Some(ref name) = base_ident
                    && (self.env.classes.contains_key(name)
                        || self.env.structs.contains_key(name)
                        || self.env.enums.contains_key(name)
                        || self.env.actors.contains_key(name))
                {
                    is_namespace_access = !self.env.is_local(*name);
                }

                // Allow :: ONLY on namespaces, and . ONLY on instances.
                // Exception: allow . on namespaces ONLY if it's NOT an enum variant (for backwards compatibility while we transition)
                if *is_static_operator && !is_namespace_access {
                    {
                        self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                        message: format!("The '::' operator can only be used for static or namespace access (object was {:?}, base_ident {:?}, classes={:?})", object, base_ident, self.env.classes.keys())
                    });
                        return Type::Error;
                    };
                }
                if !*is_static_operator && is_namespace_access {
                    {
                        self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                        message: "The '.' operator can only be used for instance access. Use '::' for static/namespace access.".into()
                    });
                        return Type::Error;
                    };
                }

                let obj_ty = self.check_expr(*object);

                let (class_name, fields, static_fields, methods) = match obj_ty {
                    Type::Class(ref name) => {
                        let sig = match self.env.classes.get(name) {
                            Some(s) => s,
                            None => {
                                self.errors.push(TypeError::Generic {
                                    src: self.get_source(),
                                    span: self.current_span,
                                    message: format!("Type '{}' is not defined", name),
                                });
                                return Type::Error;
                            }
                        };
                        (
                            *name,
                            sig.fields.clone(),
                            sig.static_fields.clone(),
                            sig.methods.clone(),
                        )
                    }
                    Type::Actor(ref name) => {
                        let sig = match self.env.actors.get(name) {
                            Some(s) => s,
                            None => {
                                self.errors.push(TypeError::Generic {
                                    src: self.get_source(),
                                    span: self.current_span,
                                    message: format!("Actor '{}' is not defined", name),
                                });
                                return Type::Error;
                            }
                        };
                        (
                            *name,
                            sig.fields.clone(),
                            sig.static_fields.clone(),
                            sig.methods.clone(),
                        )
                    }
                    Type::Struct(ref name) => {
                        let sig = match self.env.structs.get(name) {
                            Some(s) => s,
                            None => {
                                self.errors.push(TypeError::Generic {
                                    src: self.get_source(),
                                    span: self.current_span,
                                    message: format!("Type '{}' is not defined", name),
                                });
                                return Type::Error;
                            }
                        };
                        (
                            *name,
                            sig.fields.clone(),
                            sig.static_fields.clone(),
                            sig.methods.clone(),
                        )
                    }
                    Type::Enum(ref name) => {
                        let sig = match self.env.enums.get(name) {
                            Some(s) => s,
                            None => {
                                self.errors.push(TypeError::Generic {
                                    src: self.get_source(),
                                    span: self.current_span,
                                    message: format!("Enum '{}' is not defined", name),
                                });
                                return Type::Error;
                            }
                        };
                        if sig.variants.contains_key(property) {
                            return Type::Enum(*name);
                        }
                        {
                            self.errors.push(TypeError::Generic {
                                src: self.get_source(),
                                span: self.current_span,
                                message: format!("Enum '{}' has no variant '{}'", name, property),
                            });
                            return Type::Error;
                        };
                    }
                    _ => {
                        {
                            self.errors.push(TypeError::Generic {
                                src: self.get_source(),
                                span: self.current_span,
                                message: format!(
                                    "Cannot access property '{}' on non-object type",
                                    property
                                ),
                            });
                            return Type::Error;
                        };
                    }
                };

                if let Some(ty) = static_fields.get(&ustr::Ustr::from(property)) {
                    return ty.clone();
                }
                if let Some(ty) = fields.get(&ustr::Ustr::from(property)) {
                    if let Type::Actor(ref a_name) = obj_ty
                        && Some(*a_name) != self.current_class
                    {
                        {
                            self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                                message: format!("Actor fields are isolated and cannot be accessed from outside actor '{}'", a_name.split("__").last().unwrap_or(a_name))
                            });
                            return Type::Error;
                        };
                    }
                    return ty.clone();
                }
                if let Some(m_sig) = methods.get(&ustr::Ustr::from(property)) {
                    if m_sig.visibility == Visibility::Private
                        && self.current_class.as_deref() != Some(&*class_name)
                    {
                        {
                            self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                                message: format!("Method '{}' is private and cannot be accessed from outside class/actor '{}'", property, class_name.split("__").last().unwrap_or(&class_name))
                            });
                            return Type::Error;
                        };
                    }
                    if matches!(obj_ty, Type::Actor(_)) {
                        return Type::Promise(Box::new(m_sig.return_type.clone()));
                    }
                    return m_sig.return_type.clone();
                }
                {
                    self.errors.push(TypeError::Generic {
                        src: self.get_source(),
                        span: self.current_span,
                        message: format!(
                            "Property '{}' not found on type '{}'",
                            property, class_name
                        ),
                    });
                    Type::Error
                }
            }
            Expr::Await(inner) => {
                let inner_ty = self.check_expr(*inner);
                if let Type::Promise(t) = inner_ty {
                    *t
                } else {
                    {
                        self.errors.push(TypeError::Generic {
                            src: self.get_source(),
                            span: self.current_span,
                            message: "Cannot await a non-promise type".into(),
                        });
                        Type::Error
                    }
                }
            }
            Expr::Unwrap(inner) => {
                let inner_ty = self.check_expr(*inner);
                if let Type::Nullable(t) = inner_ty {
                    *t
                } else {
                    {
                        self.errors.push(TypeError::Generic {
                            src: self.get_source(),
                            span: self.current_span,
                            message: "Cannot unwrap a non-nullable type".into(),
                        });
                        Type::Error
                    }
                }
            }
            Expr::Try(inner) => {
                let inner_ty = self.check_expr(*inner);
                if let Type::Enum(name) = &inner_ty
                    && let Some(sig) = self.env.enums.get(name)
                {
                    if name.starts_with("Result_") {
                        if let Some(Type::Enum(ret_name)) = &self.current_return_type {
                            if !ret_name.starts_with("Result_") {
                                {
                                    self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span, message: "Cannot use ? on a Result in a function that does not return Result".into() });
                                    return Type::Error;
                                };
                            }
                        } else {
                            {
                                self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span, message: "Cannot use ? on a Result in a function that does not return Result".into() });
                                return Type::Error;
                            };
                        }
                        if let Some(Some(fields)) = sig.variants.get(&ustr::Ustr::from("Ok"))
                            && let Some(t) = fields.first()
                        {
                            return t.clone();
                        }
                        return Type::Void;
                    } else if name.starts_with("Option_") {
                        if let Some(Type::Enum(ret_name)) = &self.current_return_type {
                            if !ret_name.starts_with("Option_") {
                                {
                                    self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span, message: "Cannot use ? on an Option in a function that does not return Option".into() });
                                    return Type::Error;
                                };
                            }
                        } else {
                            {
                                self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span, message: "Cannot use ? on an Option in a function that does not return Option".into() });
                                return Type::Error;
                            };
                        }
                        if let Some(Some(fields)) = sig.variants.get(&ustr::Ustr::from("Some"))
                            && let Some(t) = fields.first()
                        {
                            return t.clone();
                        }
                        return Type::Void;
                    }
                }
                {
                    self.errors.push(TypeError::Generic {
                        src: self.get_source(),
                        span: self.current_span,
                        message: "The ? operator can only be applied to Result or Option types"
                            .to_string(),
                    });
                    Type::Error
                }
            }
            Expr::NullCoalesce { left, right } => {
                let left_ty = self.check_expr(*left);
                let right_ty = self.check_expr(*right);
                if let Type::Nullable(inner) = left_ty {
                    if *inner == right_ty {
                        *inner
                    } else if right_ty == Type::Null {
                        Type::Nullable(inner)
                    } else {
                        {
                            self.errors.push(TypeError::Generic {
                                src: self.get_source(),
                                span: self.current_span,
                                message: format!(
                                    "Null coalesce type mismatch: {:?} and {:?}",
                                    *inner, right_ty
                                ),
                            });
                            Type::Error
                        }
                    }
                } else {
                    {
                        self.errors.push(TypeError::Generic {
                            src: self.get_source(),
                            span: self.current_span,
                            message: "Left side of ?? must be nullable".into(),
                        });
                        Type::Error
                    }
                }
            }
            Expr::OptionalMemberAccess { object, property } => {
                let obj_ty = self.check_expr(*object);
                if let Type::Nullable(inner) = obj_ty {
                    // Check property on inner type

                    // Instead of full check, we can manually check if it's Class or Struct
                    let (class_name, sig) = match &*inner {
                        Type::Class(name) => (name, self.env.classes.get(name).unwrap()),
                        Type::Struct(name) => (name, self.env.structs.get(name).unwrap()),
                        _ => {
                            self.errors.push(TypeError::Generic {
                                src: self.get_source(),
                                span: self.current_span,
                                message: "Optional access on non-object".into(),
                            });
                            return Type::Error;
                        }
                    };

                    if let Some(f_ty) = sig.fields.get(&ustr::Ustr::from(property)) {
                        return Type::Nullable(Box::new(f_ty.clone()));
                    }
                    if let Some(m_sig) = sig.methods.get(&ustr::Ustr::from(property)) {
                        return Type::Nullable(Box::new(m_sig.return_type.clone()));
                    }
                    {
                        self.errors.push(TypeError::Generic {
                            src: self.get_source(),
                            span: self.current_span,
                            message: format!(
                                "Property '{}' not found on type '{}'",
                                property, class_name
                            ),
                        });
                        Type::Error
                    }
                } else {
                    {
                        self.errors.push(TypeError::Generic {
                            src: self.get_source(),
                            span: self.current_span,
                            message: "Optional member access on non-nullable type".into(),
                        });
                        Type::Error
                    }
                }
            }
        }
    }
}
