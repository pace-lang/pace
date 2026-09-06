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
                if let Some(sig) = self.env.classes.get(name) {
                    if sig.visibility == pace_ast::Visibility::Private && sig.module != self.current_module {
                        self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span, message: format!("Class '{}' is private", name.split("__").last().unwrap_or(name)) });
                        return Type::Error;
                    }
                    Type::Class(*name)
                } else if let Some(sig) = self.env.actors.get(name) {
                    if sig.visibility == pace_ast::Visibility::Private && sig.module != self.current_module {
                        self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span, message: format!("Actor '{}' is private", name.split("__").last().unwrap_or(name)) });
                        return Type::Error;
                    }
                    Type::Actor(*name)
                } else if let Some(sig) = self.env.structs.get(name) {
                    if sig.visibility == pace_ast::Visibility::Private && sig.module != self.current_module {
                        self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span, message: format!("Struct '{}' is private", name.split("__").last().unwrap_or(name)) });
                        return Type::Error;
                    }
                    Type::Struct(*name)
                } else if let Some(sig) = self.env.enums.get(name) {
                    if sig.visibility == pace_ast::Visibility::Private && sig.module != self.current_module {
                        self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span, message: format!("Enum '{}' is private", name.split("__").last().unwrap_or(name)) });
                        return Type::Error;
                    }
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
            Expr::ArrayLiteral(elements) => {
                let list_class = ustr::Ustr::from("pace_collections_list__List");
                if elements.is_empty() {
                    Type::GenericInstance {
                        base: Box::new(Type::Class(list_class)),
                        args: vec![Type::Unknown],
                    }
                } else {
                    let first_ty = self.check_expr(elements[0]);
                    for elem in elements.iter().skip(1) {
                        let ty = self.check_expr(*elem);
                        if !self.is_assignable_to(&ty, &first_ty) {
                            self.errors.push(TypeError::Generic {
                                src: self.get_source(),
                                span: self.current_span,
                                message: format!("Array literal contains mismatched types: expected {:?}, found {:?}", first_ty, ty),
                            });
                        }
                    }
                    Type::GenericInstance {
                        base: Box::new(Type::Class(list_class)),
                        args: vec![first_ty],
                    }
                }
            }
            Expr::MapLiteral(elements) => {
                let map_class = ustr::Ustr::from("pace_collections_map__Map");
                if elements.is_empty() {
                    Type::GenericInstance {
                        base: Box::new(Type::Class(map_class)),
                        args: vec![Type::Unknown, Type::Unknown],
                    }
                } else {
                    let (first_k_id, first_v_id) = &elements[0];
                    let first_k_ty = self.check_expr(*first_k_id);
                    let first_v_ty = self.check_expr(*first_v_id);
                    
                    for (k, v) in elements.iter().skip(1) {
                        let k_ty = self.check_expr(*k);
                        let v_ty = self.check_expr(*v);
                        if !self.is_assignable_to(&k_ty, &first_k_ty) {
                            self.errors.push(TypeError::Generic {
                                src: self.get_source(),
                                span: self.current_span,
                                message: format!("Map literal contains mismatched key types: expected {:?}, found {:?}", first_k_ty, k_ty),
                            });
                        }
                        if !self.is_assignable_to(&v_ty, &first_v_ty) {
                            self.errors.push(TypeError::Generic {
                                src: self.get_source(),
                                span: self.current_span,
                                message: format!("Map literal contains mismatched value types: expected {:?}, found {:?}", first_v_ty, v_ty),
                            });
                        }
                    }
                    Type::GenericInstance {
                        base: Box::new(Type::Class(map_class)),
                        args: vec![first_k_ty, first_v_ty],
                    }
                }
            }
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
                            self.coerce_expr_if_needed(&var_info.ty, &val_ty, *value);
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

                // If callee is a known class/struct/actor, it's a constructor call
                let mut is_instantiation = false;
                let mut class_name = None;
                let mut generic_args = None;
                let mut is_actor = false;
                
                match &callee_ty {
                    Type::Class(name) => {
                        is_instantiation = true;
                        class_name = Some(*name);
                    }
                    Type::Actor(name) => {
                        is_instantiation = true;
                        class_name = Some(*name);
                        is_actor = true;
                    }
                    Type::Struct(name) => {
                        is_instantiation = true;
                        class_name = Some(*name);
                    }
                    Type::GenericInstance { base, args: g_args } => {
                        if let Type::Class(name) | Type::Struct(name) | Type::Actor(name) = &**base {
                            is_instantiation = true;
                            class_name = Some(*name);
                            generic_args = Some(g_args.clone());
                            if let Type::Actor(_) = &**base {
                                is_actor = true;
                            }
                        }
                    }
                    _ => {}
                }

                if is_instantiation {
                    if let Some(name) = class_name {
                        let init_sig = if let Some(sig) = self.env.classes.get(&name) {
                            sig.methods.get(&ustr::Ustr::from("init")).cloned()
                        } else if let Some(sig) = self.env.structs.get(&name) {
                            sig.methods.get(&ustr::Ustr::from("init")).cloned()
                        } else if let Some(sig) = self.env.actors.get(&name) {
                            sig.methods.get(&ustr::Ustr::from("init")).cloned()
                        } else {
                            None
                        };

                        if let Some(mut sig) = init_sig {
                            // Substitute generics if necessary
                            if let Some(g_args) = &generic_args {
                                let g_params = if is_actor {
                                    self.env.actors.get(&name).and_then(|def| def.generic_params.as_ref())
                                } else if self.env.classes.contains_key(&name) {
                                    self.env.classes.get(&name).and_then(|def| def.generic_params.as_ref())
                                } else {
                                    self.env.structs.get(&name).and_then(|def| def.generic_params.as_ref())
                                };
                                
                                if let Some(g_params) = g_params {
                                    let mut substs = std::collections::HashMap::new();
                                    if g_params.len() == g_args.len() {
                                        for (p, arg) in g_params.iter().zip(g_args.iter()) {
                                            substs.insert(p.name, arg.clone());
                                        }
                                        for p in sig.params.iter_mut() {
                                            *p = p.resolve_generics(&substs);
                                        }
                                    }
                                }
                            }
                            
                            if sig.params.len() != args.len() {
                                self.errors.push(TypeError::Generic {
                                    src: self.get_source(),
                                    span: self.current_span,
                                    message: format!(
                                        "Constructor for '{}' expects {} arguments, got {}",
                                        name,
                                        sig.params.len(),
                                        args.len()
                                    ),
                                });
                                return Type::Error;
                            }
                            
                            for (i, arg_ty) in arg_types.iter().enumerate() {
                                let expected_ty = &sig.params[i];
                                if expected_ty != &Type::Any && !self.is_assignable_to(arg_ty, expected_ty) && arg_ty != &Type::Unknown {
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
                        } else {
                            if !args.is_empty() {
                                self.errors.push(TypeError::Generic {
                                    src: self.get_source(),
                                    span: self.current_span,
                                    message: format!(
                                        "Type '{}' has no init() method, so it must be instantiated with 0 arguments",
                                        name
                                    ),
                                });
                                return Type::Error;
                            }
                        }
                    }
                    return callee_ty;
                } else if let Type::Enum(name) = &callee_ty {
                    if let Some(_sig) = self.env.enums.get(name) {
                        return Type::Enum(*name);
                    }
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
                        if expected_ty != &Type::Any && !self.is_assignable_to(arg_ty, expected_ty) && arg_ty != &Type::Unknown {
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
                        if expected_ty != &Type::Any && !self.is_assignable_to(arg_ty, expected_ty) && arg_ty != &Type::Unknown {
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
            } => {
                let obj_ty = self.check_expr(*object);

                // Universal toString() contract
                if property == "toString" {
                    return Type::Function {
                        generic_params: None,
                        params: vec![],
                        return_type: Box::new(Type::String),
                    };
                }

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
                    Type::GenericInstance { ref base, ref args } => {
                        if let Type::Class(ref name) = **base {
                            if let Some(sig) = self.env.classes.get(name) {
                                // Substitute class generics in methods/fields
                                let mut substs = std::collections::HashMap::new();
                                if let Some(g_params) = &sig.generic_params {
                                    for (p, arg) in g_params.iter().zip(args.iter()) {
                                        substs.insert(p.name, arg.clone());
                                    }
                                }
                                
                                let mut resolved_methods = sig.methods.clone();
                                for (_, m_sig) in resolved_methods.iter_mut() {
                                    for p in m_sig.params.iter_mut() {
                                        *p = p.resolve_generics(&substs);
                                    }
                                    m_sig.return_type = m_sig.return_type.resolve_generics(&substs);
                                }
                                
                                let mut resolved_fields = sig.fields.clone();
                                for (_, f_sig) in resolved_fields.iter_mut() {
                                    f_sig.ty = f_sig.ty.resolve_generics(&substs);
                                }
                                
                                (
                                    *name,
                                    resolved_fields,
                                    sig.static_fields.clone(), // statics usually don't depend on instance generics
                                    resolved_methods,
                                )
                            } else {
                                // if it's already monomorphized, we might find the concrete class
                                let mut concrete_name = name.as_str().to_string();
                                for arg in args {
                                    let arg_name = format!("{:?}", arg);
                                    concrete_name.push('_');
                                    concrete_name.push_str(&arg_name.replace(" ", "_"));
                                }
                                
                                if let Some(s) = self.env.classes.get(&ustr::Ustr::from(concrete_name.as_str())) {
                                    (
                                        ustr::Ustr::from(concrete_name.as_str()),
                                        s.fields.clone(),
                                        s.static_fields.clone(),
                                        s.methods.clone(),
                                    )
                                } else {
                                    self.errors.push(TypeError::Generic {
                                        src: self.get_source(),
                                        span: self.current_span,
                                        message: format!("Type '{}' is not defined", name),
                                    });
                                    return Type::Error;
                                }
                            }
                        } else if let Type::Interface(ref name) = **base {
                            if let Some(sig) = self.env.interfaces.get(name) {
                                let mut substs = std::collections::HashMap::new();
                                if let Some(g_params) = &sig.generic_params {
                                    for (p, arg) in g_params.iter().zip(args.iter()) {
                                        substs.insert(p.name, arg.clone());
                                    }
                                }
                                
                                let mut resolved_methods = sig.methods.clone();
                                for (_, m_sig) in resolved_methods.iter_mut() {
                                    for p in m_sig.params.iter_mut() {
                                        *p = p.resolve_generics(&substs);
                                    }
                                    m_sig.return_type = m_sig.return_type.resolve_generics(&substs);
                                }
                                
                                (
                                    *name,
                                    std::collections::HashMap::new(),
                                    std::collections::HashMap::new(),
                                    resolved_methods,
                                )
                            } else {
                                let mut concrete_name = name.as_str().to_string();
                                for arg in args {
                                    let arg_name = format!("{:?}", arg);
                                    concrete_name.push('_');
                                    concrete_name.push_str(&arg_name.replace(" ", "_"));
                                }
                                
                                if let Some(s) = self.env.interfaces.get(&ustr::Ustr::from(concrete_name.as_str())) {
                                    (
                                        ustr::Ustr::from(concrete_name.as_str()),
                                        std::collections::HashMap::new(),
                                        std::collections::HashMap::new(),
                                        s.methods.clone(),
                                    )
                                } else {
                                    self.errors.push(TypeError::Generic {
                                        src: self.get_source(),
                                        span: self.current_span,
                                        message: format!("Interface '{}' is not defined", name),
                                    });
                                    return Type::Error;
                                }
                            }
                        } else {
                            self.errors.push(TypeError::Generic {
                                src: self.get_source(),
                                span: self.current_span,
                                message: "Cannot access property on non-class/interface generic instance".into(),
                            });
                            return Type::Error;
                        }
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
                    Type::Interface(ref name) => {
                        let sig = match self.env.interfaces.get(name) {
                            Some(s) => s,
                            None => {
                                self.errors.push(TypeError::Generic {
                                    src: self.get_source(),
                                    span: self.current_span,
                                    message: format!("Interface '{}' is not defined", name),
                                });
                                return Type::Error;
                            }
                        };
                        (
                            *name,
                            std::collections::HashMap::new(),
                            std::collections::HashMap::new(),
                            sig.methods.clone(),
                        )
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
                    if ty.visibility == pace_ast::Visibility::Private
                        && self.current_class.as_deref() != Some(&*class_name)
                    {
                        {
                            self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                                message: format!("Static field '{}' is private and cannot be accessed from outside class/actor '{}'", property, class_name.split("__").last().unwrap_or(&class_name))
                            });
                            return Type::Error;
                        };
                    }
                    return ty.ty.clone();
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
                    if ty.visibility == pace_ast::Visibility::Private
                        && self.current_class.as_deref() != Some(&*class_name)
                    {
                        {
                            self.errors.push(TypeError::Generic { src: self.get_source(), span: self.current_span,
                                message: format!("Field '{}' is private and cannot be accessed from outside class/actor '{}'", property, class_name.split("__").last().unwrap_or(&class_name))
                            });
                            return Type::Error;
                        };
                    }
                    return ty.ty.clone();
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
                    return Type::Function {
                        generic_params: m_sig.generic_params.clone(),
                        params: m_sig.params.clone(),
                        return_type: Box::new(if matches!(obj_ty, Type::Actor(_)) {
                            Type::Promise(Box::new(m_sig.return_type.clone()))
                        } else {
                            m_sig.return_type.clone()
                        }),
                    };
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
                        return Type::Nullable(Box::new(f_ty.ty.clone()));
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

    pub(crate) fn coerce_expr_if_needed(&mut self, expected: &Type, actual: &Type, expr_id: pace_hir::ExprId) {
        let expected_str = format!("{:?}", expected);
        let actual_str = format!("{:?}", actual);
        
        if actual_str.contains("List") && (expected_str.contains("Set") || expected_str.contains("Queue") || expected_str.contains("TreeSet")) {
            if matches!(self.arena.get_expr(expr_id), Expr::ArrayLiteral(_)) {
                self.env.node_types.insert(expr_id, expected.clone());
            }
        }
        if actual_str.contains("Map") && expected_str.contains("Map") {
            if matches!(self.arena.get_expr(expr_id), Expr::MapLiteral(_)) {
                self.env.node_types.insert(expr_id, expected.clone());
            }
        }
    }
}
