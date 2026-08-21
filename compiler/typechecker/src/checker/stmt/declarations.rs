use super::*;
use ast::EnumVariant;
use session::types::Type;

impl<'a> TypeChecker<'a> {
    pub(crate) fn check_class_decl(
        &mut self,
        name: &session::Symbol,
        type_params: &[session::Symbol],
        implements: &[TypeExpr<'a>],
        methods: &[Stmt<'a>],
        fields: &[Stmt<'a>],
        _is_private: bool,
        is_actor: bool,
        span: Span,
    ) -> TypedStmtKind<'a> {
        if !type_params.is_empty() {
            return TypedStmtKind::Block(Vec::new());
        }

        self.env.push_scope();
        for tp in type_params {
            self.env.declare(
                *tp,
                self.session.types.borrow_mut().intern(Type::Generic(*tp)),
            );
        }

        // Note: Class implements validation is kept here since it emits diagnostics
        if let Some(class_members) = self.classes.get(name).cloned()
            && let Some(resolved_implements) = self.class_implements.get(name).cloned()
        {
            for imp_ty in resolved_implements {
                let interface_name = match self.get_type(imp_ty) {
                    Type::Interface(n, _) => n,
                    Type::GenericInstance(n, _) => n,
                    _ => continue,
                };
                if let Some(interface_members) = self.interfaces.get(&interface_name).cloned() {
                    for (i_method_name, i_method_ty) in interface_members {
                        if let Some(c_method_ty) = class_members.get(&i_method_name) {
                            if let Type::GenericInstance(_, _) = self.get_type(imp_ty) {
                                continue;
                            }
                            if *c_method_ty != i_method_ty {
                                self.error(span, DiagnosticCode::TypeMismatch, &format!("Class '{}' incorrectly implements method '{}' of interface '{}'. Expected '{}', found '{}'.", self.session.interner.borrow().lookup(*name), self.session.interner.borrow().lookup(i_method_name), self.session.interner.borrow().lookup(interface_name), self.session.format_type(i_method_ty), self.session.format_type(*c_method_ty)));
                            }
                        } else {
                            self.error(span, DiagnosticCode::TypeMismatch, &format!("Class '{}' does not implement required method '{}' of interface '{}'.", self.session.interner.borrow().lookup(*name), self.session.interner.borrow().lookup(i_method_name), self.session.interner.borrow().lookup(interface_name)))
                        }
                    }
                } else if self.generic_registry.get_interface(interface_name).is_some() {
                    continue;
                } else {
                    self.error(
                        span,
                        DiagnosticCode::UnknownType,
                        &format!(
                            "Interface '{}' not found.",
                            self.session.interner.borrow().lookup(interface_name)
                        ),
                    )
                }
            }
        }

        let prev_class = self.current_class;
        self.current_class = Some(*name);

        let mut typed_methods = Vec::new();
        for method in methods {
            let prev_method = self.is_checking_method;
            let prev_actor = self.is_checking_actor;
            self.is_checking_method = true;
            if is_actor {
                self.is_checking_actor = true;
            }
            typed_methods.push(self.check_stmt(method));
            self.is_checking_method = prev_method;
            self.is_checking_actor = prev_actor;
        }

        let mut typed_fields = Vec::new();
        for field in fields {
            typed_fields.push(self.check_stmt(field));
        }

        self.env.pop_scope();
        self.current_class = prev_class;
        TypedStmtKind::Class {
            name: *name,
            type_params: type_params.to_vec(),
            implements: implements.to_vec(),
            methods: typed_methods,
            fields: typed_fields,
            is_actor,
        }
    }

    pub(crate) fn check_struct_decl(
        &mut self,
        name: &session::Symbol,
        type_params: &[session::Symbol],
        methods: &[Stmt<'a>],
        fields: &[Stmt<'a>],
        _is_private: bool,
        span: Span,
    ) -> TypedStmtKind<'a> {
        if !type_params.is_empty() {
            return TypedStmtKind::Block(Vec::new());
        }

        self.env.push_scope();
        for tp in type_params {
            self.env.declare(
                *tp,
                self.session.types.borrow_mut().intern(Type::Generic(*tp)),
            );
        }

        if let Some(struct_members) = self.classes.get(name).cloned() {
            for (field_name, field_ty_id) in struct_members {
                if matches!(self.get_type(field_ty_id), Type::Function(..)) {
                    continue;
                }

                let is_valid = match self.get_type(field_ty_id) {
                    Type::Int | Type::Float | Type::Bool | Type::Error | Type::Any => true,
                    Type::Instance(name) => self.classes.contains_key(&name),
                    _ => false,
                };

                if !is_valid {
                    self.error(span, DiagnosticCode::TypeMismatch, &format!("Struct '{}' cannot contain field '{}' of type '{}'. Structs can only contain primitives (Int, Float, Bool) or other structs.", self.session.interner.borrow().lookup(*name), self.session.interner.borrow().lookup(field_name), self.session.format_type(field_ty_id)));
                }
            }
        }

        let prev_class = self.current_class;
        self.current_class = Some(*name);

        let mut typed_methods = Vec::new();
        for method in methods {
            let prev = self.is_checking_method;
            self.is_checking_method = true;
            typed_methods.push(self.check_stmt(method));
            self.is_checking_method = prev;
        }

        let mut typed_fields = Vec::new();
        for field in fields {
            typed_fields.push(self.check_stmt(field));
        }

        self.env.pop_scope();
        self.current_class = prev_class;
        TypedStmtKind::Struct {
            name: *name,
            type_params: type_params.to_vec(),
            methods: typed_methods,
            fields: typed_fields,
        }
    }

    pub(crate) fn check_enum_decl(
        &mut self,
        name: &session::Symbol,
        type_params: &[session::Symbol],
        variants: &[EnumVariant<'a>],
        methods: &[Stmt<'a>],
        _is_private: bool,
    ) -> TypedStmtKind<'a> {
        self.env.push_scope();
        for tp in type_params {
            self.env.declare(
                *tp,
                self.session.types.borrow_mut().intern(Type::Generic(*tp)),
            );
        }

        let prev_class = self.current_class;
        self.current_class = Some(*name);

        let mut typed_methods = Vec::new();
        for method in methods {
            let prev = self.is_checking_method;
            self.is_checking_method = true;
            typed_methods.push(self.check_stmt(method));
            self.is_checking_method = prev;
        }

        self.env.pop_scope();
        self.current_class = prev_class;
        TypedStmtKind::Enum {
            name: *name,
            type_params: type_params.to_vec(),
            variants: variants.to_vec(),
            methods: typed_methods,
        }
    }

    pub(crate) fn check_extension_decl(
        &mut self,
        target_type: &TypeExpr<'a>,
        type_params: &[session::Symbol],
        methods: &[Stmt<'a>],
        span: Span,
    ) -> TypedStmtKind<'a> {
        if !type_params.is_empty() {
            return TypedStmtKind::Block(Vec::new());
        }

        self.env.push_scope();
        let target_id = self.parse_type(target_type, span);

        let prev_class = self.current_class;
        self.env
            .declare(self.session.interner.borrow_mut().intern("self"), target_id);

        let mut typed_methods = Vec::new();
        for method in methods {
            self.is_checking_method = true;
            typed_methods.push(self.check_stmt(method));
            self.is_checking_method = false;
        }

        self.env.pop_scope();
        self.current_class = prev_class;

        TypedStmtKind::Extension {
            target_type: target_id,
            methods: typed_methods,
        }
    }

    pub(crate) fn check_func_decl(
        &mut self,
        name: &session::Symbol,
        type_params: &[session::Symbol],
        params: &[(session::Symbol, TypeExpr<'a>)],
        return_type: &Option<TypeExpr<'a>>,
        body: &'a Stmt<'a>,
        _is_private: bool,
        is_async: bool,
        is_static: bool,
        span: Span,
    ) -> TypedStmtKind<'a> {
        if !type_params.is_empty() || self.generic_registry.get_function(*name).is_some() {
            return TypedStmtKind::Block(Vec::new());
        }

        let mut ret_ty = if let Some(rt) = return_type {
            self.parse_type(rt, span)
        } else {
            self.session.types.borrow_mut().intern(Type::Void)
        };

        let is_method = self.is_checking_method;
        if (is_async || (is_method && self.is_checking_actor)) && !matches!(self.get_type(ret_ty), Type::Task(_)) {
            ret_ty = self.session.types.borrow_mut().intern(Type::Task(ret_ty));
        }

        let mut param_types = Vec::new();
        for (_, param_type_str) in params {
            param_types.push(self.parse_type(param_type_str, span));
        }

        let actually_async = is_async || (is_method && self.is_checking_actor);
        self.is_checking_method = false;

        let mut resolved_name = *name;
        if !is_method
            && let Some(existing) = self.env.resolve(*name)
            && matches!(self.get_type(existing), Type::OverloadedFunction(..))
        {
            let mut mangled = format!("_PO_{}", self.session.interner.borrow().lookup(*name));
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
            resolved_name = self.session.interner.borrow_mut().intern(&mangled);
        }

        self.env.push_scope();
        for tp in type_params {
            self.env.declare(
                *tp,
                self.session.types.borrow_mut().intern(Type::Generic(*tp)),
            );
        }

        if let Some(ref class_name) = self.current_class {
            self.env.declare_var(
                self.session.interner.borrow_mut().intern("self"),
                self.session
                    .types
                    .borrow_mut()
                    .intern(Type::Instance(*class_name)),
                false,
            );
        }

        for ((param_name, _), param_ty) in params.iter().zip(param_types) {
            self.env.declare_var(*param_name, param_ty, false);
        }

        let previous_return = self.current_return_type.take();
        self.current_return_type = Some(ret_ty);

        let previous_in_async_context = self.in_async_context;
        if actually_async {
            self.in_async_context = true;
        }

        let typed_body = self.check_stmt(body);

        self.in_async_context = previous_in_async_context;

        if self.session.interner.borrow().lookup(*name) == "init"
            && let Some(ref class_name) = self.current_class
            && let Some(uninit_props_ref) = self.uninitialized_class_properties.get(class_name)
        {
            let uninit_props = uninit_props_ref.clone();
            let assigned_props = Self::get_assigned_properties_in_init(&typed_body);
            for prop in uninit_props {
                if !assigned_props.contains(&prop) {
                    self.error(
                        span,
                        DiagnosticCode::UninitializedVariable,
                        &format!(
                            "Property '{}' is not initialized by the constructor.",
                            self.session.interner.borrow().lookup(prop)
                        ),
                    );
                }
            }
        }

        self.current_return_type = previous_return;
        self.env.pop_scope();
        TypedStmtKind::Func {
            name: resolved_name,
            type_params: type_params.to_vec(),
            params: params.to_vec(),
            return_type: return_type.clone(),
            body: self.alloc(typed_body),
            is_async: actually_async,
            is_static,
        }
    }

    pub(crate) fn check_var_decl(
        &mut self,
        name: session::Symbol,
        type_annotation: &Option<TypeExpr<'a>>,
        initializer: &Option<&Expr<'a>>,
        is_weak: bool,
        is_mutable: bool,
        is_static: bool,
        span: Span,
    ) -> TypedStmt<'a> {
        let expected_ty = type_annotation
            .as_ref()
            .map(|ann| self.parse_type(ann, span));

        let typed_init = initializer
            .as_ref()
            .map(|init| self.check_expr_with_expected(init, expected_ty));

        let init_type = typed_init
            .as_ref()
            .map(|t| t.ty)
            .unwrap_or(self.session.types.borrow_mut().intern(Type::Any));

        let decl_type = if let Some(ann_type) = expected_ty {
            if init_type != self.session.types.borrow_mut().intern(Type::Any)
                && !self.is_assignable(init_type, ann_type)
                && init_type != self.session.types.borrow_mut().intern(Type::Error)
            {
                self.error(
                    span,
                    DiagnosticCode::TypeMismatch,
                    &format!(
                        "Cannot assign type '{}' to variable of type '{}'.",
                        self.session.format_type(init_type),
                        self.session.format_type(ann_type)
                    ),
                );
            }

            if is_weak
                && !matches!(self.session.types.borrow().get(ann_type), Type::Optional(inner) if matches!(self.session.types.borrow().get(*inner), Type::Instance(_) | Type::Interface(_, _)))
            {
                self.error(
                    span,
                    DiagnosticCode::TypeMismatch,
                    "Weak variables must be of optional instance type (e.g. 'weak var x: User?').",
                );
            }
            ann_type
        } else {
            if is_weak
                && !matches!(self.session.types.borrow().get(init_type), Type::Optional(inner) if matches!(self.session.types.borrow().get(*inner), Type::Instance(_) | Type::Interface(_, _)))
            {
                self.error(
                    span,
                    DiagnosticCode::TypeMismatch,
                    "Weak variables must be of optional instance type (e.g. 'weak var x: User?').",
                );
            }
            init_type
        };

        self.env.declare_var(name, decl_type, is_mutable);

        let kind = if is_weak || (initializer.is_none() && type_annotation.is_some()) {
            TypedStmtKind::Var {
                name,
                type_annotation: type_annotation.clone(),
                initializer: typed_init.map(|e| self.alloc(e)),
                is_weak,
                is_static,
            }
        } else {
            TypedStmtKind::Let {
                name,
                type_annotation: type_annotation.clone(),
                initializer: typed_init.map(|e| self.alloc(e)),
                is_static,
            }
        };
        TypedStmt::new(kind, span)
    }
}
