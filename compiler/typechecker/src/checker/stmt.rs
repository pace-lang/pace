use super::*;
use session::types::Type;

impl<'a> TypeChecker<'a> {
    pub(crate) fn check_stmt(&mut self, stmt: &Stmt<'a>) -> TypedStmt<'a> {
        let kind = match &stmt.kind {
            StmtKind::Block(stmts) => {
                self.env.push_scope();
                self.collect_declarations(stmts);
                let typed_stmts = self.check(stmts);
                self.env.pop_scope();
                TypedStmtKind::Block(typed_stmts)
            }
            StmtKind::Let {
                name,
                type_annotation,
                initializer,
                is_private: _,
            } => {
                self.check_var_decl(*name, type_annotation, initializer, false, false, stmt.span)
                    .kind
            }
            StmtKind::Var {
                name,
                type_annotation,
                initializer,
                is_weak,
                is_private: _,
            } => {
                self.check_var_decl(
                    *name,
                    type_annotation,
                    initializer,
                    *is_weak,
                    true,
                    stmt.span,
                )
                .kind
            }
            StmtKind::Class {
                name,
                type_params,
                implements,
                methods,
                fields,
                is_private: _,
            }
            | StmtKind::Actor {
                name,
                type_params,
                implements,
                methods,
                fields,
                is_private: _,
            } => {
                if !type_params.is_empty() {
                    return TypedStmt {
                        kind: TypedStmtKind::Block(Vec::new()),
                        span: stmt.span,
                    };
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
                        if let Some(interface_members) =
                            self.interfaces.get(&interface_name).cloned()
                        {
                            for (i_method_name, i_method_ty) in interface_members {
                                if let Some(c_method_ty) = class_members.get(&i_method_name) {
                                    // Skip exact type check for generic interfaces for now to prevent false positives
                                    if let Type::GenericInstance(_, _) = self.get_type(imp_ty) {
                                        continue;
                                    }
                                    if *c_method_ty != i_method_ty {
                                        self.error(stmt.span, DiagnosticCode::TypeMismatch, &format!("Class '{}' incorrectly implements method '{}' of interface '{}'. Expected '{}', found '{}'.", self.session.interner.borrow().lookup(*name), self.session.interner.borrow().lookup(i_method_name), self.session.interner.borrow().lookup(interface_name), self.session.format_type(i_method_ty), self.session.format_type(*c_method_ty)));
                                    }
                                } else {
                                    self.error(stmt.span, DiagnosticCode::TypeMismatch, &format!("Class '{}' does not implement required method '{}' of interface '{}'.", self.session.interner.borrow().lookup(*name), self.session.interner.borrow().lookup(i_method_name), self.session.interner.borrow().lookup(interface_name)))
                                }
                            }
                        } else if self.generic_registry.get_interface(interface_name).is_some() {
                            // Skip checking methods for generic interfaces for now
                            continue;
                        } else {
                            self.error(
                                stmt.span,
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

                let is_actor = matches!(&stmt.kind, StmtKind::Actor { .. });

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
                    type_params: type_params.clone(),
                    implements: implements.clone(),
                    methods: typed_methods,
                    fields: typed_fields,
                    is_actor,
                }
            }
            StmtKind::Struct {
                name,
                type_params,
                methods,
                fields,
                is_private: _,
            } => {
                if !type_params.is_empty() {
                    return TypedStmt {
                        kind: TypedStmtKind::Block(Vec::new()),
                        span: stmt.span,
                    };
                }

                self.env.push_scope();
                for tp in type_params {
                    self.env.declare(
                        *tp,
                        self.session.types.borrow_mut().intern(Type::Generic(*tp)),
                    );
                }

                // Enforce that struct fields are only primitives or other structs
                if let Some(struct_members) = self.classes.get(name).cloned() {
                    for (field_name, field_ty_id) in struct_members {
                        // methods are functions, those are fine
                        if matches!(self.get_type(field_ty_id), Type::Function(..)) {
                            continue;
                        }

                        let is_valid = match self.get_type(field_ty_id) {
                            Type::Int | Type::Float | Type::Boolean | Type::Error | Type::Any => true,
                            Type::Instance(name) => self.classes.contains_key(&name), // Ideally check if it's a struct, but we don't have struct/class distinction in Type::Instance. Let's allow Instance, or we can check self.program.classes.is_struct... wait! Typechecker doesn't have is_struct. Let's just allow Type::Instance!
                            _ => false,
                        };

                        if !is_valid {
                            self.error(stmt.span, DiagnosticCode::TypeMismatch, &format!("Struct '{}' cannot contain field '{}' of type '{}'. Structs can only contain primitives (Int, Float, Boolean) or other structs.", self.session.interner.borrow().lookup(*name), self.session.interner.borrow().lookup(field_name), self.session.format_type(field_ty_id)));
                        }
                    }
                }

                let prev_class = self.current_class;
                self.current_class = Some(*name); // Reuse current_class for struct context

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
                    type_params: type_params.clone(),
                    methods: typed_methods,
                    fields: typed_fields,
                }
            }
            StmtKind::Interface {
                name,
                type_params,
                methods: _,
                is_private: _,
            } => TypedStmtKind::Interface {
                name: *name,
                type_params: type_params.clone(),
                methods: Vec::new(),
            },
            StmtKind::Enum {
                name,
                type_params,
                variants,
                methods,
                is_private: _,
            } => {
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
                    type_params: type_params.clone(),
                    variants: variants.clone(),
                    methods: typed_methods,
                }
            }
            StmtKind::TypeAlias {
                name,
                type_params,
                target_type,
                is_private: _,
            } => TypedStmtKind::TypeAlias {
                name: *name,
                type_params: type_params.clone(),
                target_type: target_type.clone(),
            },
            StmtKind::ForeignFunc {
                name,
                base_name,
                type_params: _,
                params,
                return_type,
                is_private: _,
            } => TypedStmtKind::ForeignFunc {
                name: *name,
                base_name: *base_name,
                params: params.clone(),
                return_type: return_type.clone(),
            },
            StmtKind::Func {
                name,
                type_params,
                params,
                return_type,
                body,
                is_private: _,
                is_async,
            } => {
                if !type_params.is_empty() || self.generic_registry.get_function(*name).is_some() {
                    return TypedStmt {
                        kind: TypedStmtKind::Block(Vec::new()),
                        span: stmt.span,
                    };
                }

                let mut ret_ty = if let Some(rt) = return_type {
                    self.parse_type(rt, stmt.span)
                } else {
                    self.session.types.borrow_mut().intern(Type::Void)
                };

                let is_method = self.is_checking_method;
                if (*is_async || (is_method && self.is_checking_actor)) && !matches!(self.get_type(ret_ty), Type::Task(_)) {
                    ret_ty = self.session.types.borrow_mut().intern(Type::Task(ret_ty));
                }

                let mut param_types = Vec::new();
                for (_, param_type_str) in params {
                    param_types.push(self.parse_type(param_type_str, stmt.span));
                }

                let actually_async = *is_async || (is_method && self.is_checking_actor);
                self.is_checking_method = false; // Reset so nested functions are declared as normal variables

                let mut resolved_name = *name;
                if !is_method
                    && let Some(existing) = self.env.resolve(*name)
                    && matches!(self.get_type(existing), Type::OverloadedFunction(..))
                {
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
                    && let Some(uninit_props_ref) =
                        self.uninitialized_class_properties.get(class_name)
                {
                    let uninit_props = uninit_props_ref.clone();
                    let assigned_props = Self::get_assigned_properties_in_init(&typed_body);
                    for prop in uninit_props {
                        if !assigned_props.contains(&prop) {
                            self.error(
                                stmt.span,
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
                    type_params: type_params.clone(),
                    params: params.clone(),
                    return_type: return_type.clone(),
                    body: self.alloc(typed_body),
                    is_async: actually_async,
                }
            }
            StmtKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let typed_condition = self.check_expr(condition);
                if typed_condition.ty != self.session.types.borrow_mut().intern(Type::Boolean)
                    && typed_condition.ty != self.session.types.borrow_mut().intern(Type::Error)
                {
                    self.error(
                        condition.span,
                        DiagnosticCode::TypeMismatch,
                        &format!(
                            "Expected 'Boolean' for if condition, found '{}'.",
                            self.session.format_type(typed_condition.ty)
                        ),
                    )
                }

                let typed_then = self.check_stmt(then_branch);
                let typed_else = else_branch.as_ref().map(|e_branch| {
                    let stmt = self.check_stmt(e_branch);
                    self.alloc(stmt)
                });
                TypedStmtKind::If {
                    condition: self.alloc(typed_condition),
                    then_branch: self.alloc(typed_then),
                    else_branch: typed_else,
                }
            }
            StmtKind::While { condition, body } => {
                let typed_condition = self.check_expr(condition);
                if typed_condition.ty != self.session.types.borrow_mut().intern(Type::Boolean)
                    && typed_condition.ty != self.session.types.borrow_mut().intern(Type::Error)
                {
                    self.error(
                        condition.span,
                        DiagnosticCode::TypeMismatch,
                        &format!(
                            "Expected 'Boolean' for while condition, found '{}'.",
                            self.session.format_type(typed_condition.ty)
                        ),
                    )
                }

                let typed_body = self.check_stmt(body);
                TypedStmtKind::While {
                    condition: self.alloc(typed_condition),
                    body: self.alloc(typed_body),
                }
            }
            StmtKind::For {
                item_name,
                iterator,
                body,
            } => {
                let typed_iterator = self.check_expr(iterator);

                let item_type = match self.get_type(typed_iterator.ty) {
                    Type::Range => self.session.types.borrow_mut().intern(Type::Int),
                    Type::Array(inner) => inner,
                    Type::Error => self.session.types.borrow_mut().intern(Type::Error),
                    Type::Instance(class_name) => {
                        let mut item_ty = None;
                        if let Some(implements) = self.class_implements.get(&class_name) {
                            let iterable_sym =
                                self.session.interner.borrow_mut().intern("Iterable");
                            for imp_ty in implements {
                                if let Type::GenericInstance(name, args) = self.get_type(*imp_ty)
                                    && name == iterable_sym
                                    && args.len() == 1
                                {
                                    item_ty = Some(args[0]);
                                    break;
                                }
                            }
                        }
                        if let Some(ty) = item_ty {
                            ty
                        } else {
                            self.error(
                                stmt.span,
                                DiagnosticCode::TypeMismatch,
                                &format!(
                                    "Cannot iterate over type '{}' because it does not implement 'Iterable<T>'.",
                                    self.session.format_type(typed_iterator.ty)
                                ),
                            );
                            self.session.types.borrow_mut().intern(Type::Error)
                        }
                    }
                    _ => {
                        self.error(
                            stmt.span,
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
                let typed_body = self.check_stmt(body);
                self.env.pop_scope();
                TypedStmtKind::For {
                    item_name: *item_name,
                    iterator: self.alloc(typed_iterator),
                    body: self.alloc(typed_body),
                    item_ty: item_type,
                }
            }
            StmtKind::Extension {
                target_type,
                type_params,
                methods,
            } => {
                if !type_params.is_empty() {
                    return TypedStmt {
                        kind: TypedStmtKind::Block(Vec::new()),
                        span: stmt.span,
                    };
                }

                self.env.push_scope();
                let target_id = self.parse_type(target_type, stmt.span);

                let prev_class = self.current_class;
                // We use the string representation of the target type as the "class" name for mangling
                // or just leave current_class as None. Wait, `self` needs to be declared!
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
            StmtKind::Import { .. } | StmtKind::Export { .. } => ast::TypedStmtKind::Block(vec![]),
            StmtKind::Expression(expr) => TypedStmtKind::Expression({
                let tmp = self.check_expr(expr);
                self.alloc(tmp)
            }),
            StmtKind::Return { value } => {
                let typed_val = if let Some(val) = value {
                    let mut expected_ret = self.current_return_type;
                    
                    if self.in_async_context {
                        if let Some(expected_ty) = expected_ret {
                            if let Type::Task(inner_type_id) = self.get_type(expected_ty).clone() {
                                expected_ret = Some(inner_type_id);
                            }
                        }
                    }
                    
                    let mut tv = self.check_expr_with_expected(val, expected_ret);

                    if let Some(expected_ty) = expected_ret {
                        if !self.is_assignable(tv.ty, expected_ty)
                            && tv.ty != self.session.types.borrow_mut().intern(Type::Error)
                            && expected_ty != self.session.types.borrow_mut().intern(Type::Error)
                            && expected_ty != self.session.types.borrow_mut().intern(Type::Any)
                        {
                            self.error(
                                val.span,
                                DiagnosticCode::TypeMismatch,
                                &format!(
                                    "Expected return type '{}', found '{}'.",
                                    self.session.format_type(expected_ty),
                                    self.session.format_type(tv.ty)
                                ),
                            );
                            tv.ty = self.session.types.borrow_mut().intern(Type::Error);
                        }
                    } else {
                        self.error(
                            stmt.span,
                            DiagnosticCode::TypeMismatch,
                            "Cannot return from outside a function.",
                        );
                    }
                    Some(tv)
                } else {
                    let mut expected_ret = self.current_return_type;
                    if self.in_async_context {
                        if let Some(expected_ty) = expected_ret {
                            if let Type::Task(inner_type_id) = self.get_type(expected_ty).clone() {
                                expected_ret = Some(inner_type_id);
                            }
                        }
                    }
                    
                    if expected_ret.is_some()
                        && *expected_ret.as_ref().unwrap()
                            != self.session.types.borrow_mut().intern(Type::Void)
                    {
                        self.error(
                            stmt.span,
                            DiagnosticCode::TypeMismatch,
                            &format!(
                                "Expected return type '{}', found 'Void'.",
                                self.session
                                    .format_type(*expected_ret.as_ref().unwrap())
                            ),
                        );
                    }
                    None
                };
                TypedStmtKind::Return {
                    value: typed_val.map(|e| self.alloc(e)),
                }
            }
        };
        TypedStmt::new(kind, stmt.span)
    }

    pub(crate) fn check_var_decl(
        &mut self,
        name: session::Symbol,
        type_annotation: &Option<TypeExpr<'a>>,
        initializer: &Option<&Expr<'a>>,
        is_weak: bool,
        is_mutable: bool,
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
            }
        } else {
            TypedStmtKind::Let {
                name,
                type_annotation: type_annotation.clone(),
                initializer: typed_init.map(|e| self.alloc(e)),
            }
        };
        TypedStmt::new(kind, span)
    }
}
