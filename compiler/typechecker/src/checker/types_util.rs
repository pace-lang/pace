use super::*;
use session::types::Type;

impl<'a> TypeChecker<'a> {
    pub(crate) fn parse_type(&mut self, type_expr: &TypeExpr, span: Span) -> TypeId {
        match type_expr {
            TypeExpr::Named(name) => match self.session.interner.borrow().lookup(*name) {
                "Int" => self.session.types.borrow_mut().intern(Type::Int),
                "Float" => self.session.types.borrow_mut().intern(Type::Float),
                "String" => self.session.types.borrow_mut().intern(Type::String),
                "Boolean" => self.session.types.borrow_mut().intern(Type::Boolean),
                "Void" => self.session.types.borrow_mut().intern(Type::Void),
                "CInt" => self.session.types.borrow_mut().intern(Type::CInt),
                "CUInt" => self.session.types.borrow_mut().intern(Type::CUInt),
                "CChar" => self.session.types.borrow_mut().intern(Type::CChar),
                "CSize" => self.session.types.borrow_mut().intern(Type::CSize),
                _ => {
                    if let Some(ty_id) = self.env.resolve(*name)
                        && let Type::Generic(_g) = self.session.types.borrow().get(ty_id) {
                            return ty_id;
                        }
                    if self.classes.contains_key(name) || self.enums.contains_key(name) {
                        self.session
                            .types
                            .borrow_mut()
                            .intern(Type::Instance(*name))
                    } else if self.interfaces.contains_key(name) {
                        self.session
                            .types
                            .borrow_mut()
                            .intern(Type::Interface(*name))
                    } else {
                        self.error(
                            span,
                            DiagnosticCode::UnknownType,
                            &format!(
                                "Unknown type '{}'.",
                                self.session.interner.borrow().lookup(*name)
                            ),
                        );

                        self.session.types.borrow_mut().intern(Type::Error)
                    }
                }
            },
            TypeExpr::GenericInstance(name, args) => {
                let parsed_args = args
                    .iter()
                    .map(|a| self.parse_type(a, span))
                    .collect::<Vec<_>>();
                if self.session.interner.borrow().lookup(*name) == "Pointer"
                    && parsed_args.len() == 1
                {
                    self.session
                        .types
                        .borrow_mut()
                        .intern(Type::Pointer(parsed_args[0]))
                } else if self.classes.contains_key(name) || self.enums.contains_key(name) {
                    self.session
                        .types
                        .borrow_mut()
                        .intern(Type::GenericInstance(*name, parsed_args))
                } else {
                    self.error(
                        span,
                        DiagnosticCode::UnknownType,
                        &format!(
                            "Unknown generic class '{}'.",
                            self.session.interner.borrow().lookup(*name)
                        ),
                    );

                    self.session.types.borrow_mut().intern(Type::Error)
                }
            }
            TypeExpr::Optional(inner) => {
                let inner_parsed = self.parse_type(inner, span);
                self.session
                    .types
                    .borrow_mut()
                    .intern(Type::Optional(inner_parsed))
            }
            TypeExpr::Array(inner) => {
                let inner_parsed = self.parse_type(inner, span);
                self.session
                    .types
                    .borrow_mut()
                    .intern(Type::Array(inner_parsed))
            }
        }
    }

    pub(crate) fn is_assignable(&mut self, source: TypeId, target: TypeId) -> bool {
        let err_id = self.session.types.borrow_mut().intern(Type::Error);
        let any_id = self.session.types.borrow_mut().intern(Type::Any);
        let null_id = self.session.types.borrow_mut().intern(Type::Null);

        if source == target || source == err_id || target == err_id {
            return true;
        }

        let source_ty = self.get_type(source);
        let target_ty = self.get_type(target);

        let is_source_int = matches!(
            source_ty,
            Type::Int | Type::CInt | Type::CUInt | Type::CChar | Type::CSize
        );
        let is_target_int = matches!(
            target_ty,
            Type::Int | Type::CInt | Type::CUInt | Type::CChar | Type::CSize
        );
        if is_source_int && is_target_int {
            return true;
        }
        if source == null_id && matches!(target_ty, Type::Optional(_)) {
            return true;
        }
        if let Type::Optional(inner) = target_ty
            && self.is_assignable(source, inner) {
                return true;
            }
        if target == any_id {
            return true;
        }
        if let (Type::Instance(class_name), Type::Interface(interface_name)) =
            (source_ty, target_ty)
            && let Some(implements) = self.class_implements.get(&class_name)
                && implements.contains(&interface_name) {
                    return true;
                }
        false
    }

    pub(crate) fn substitute_generics(
        &mut self,
        ty: TypeId,
        replacements: &std::collections::HashMap<Symbol, TypeId>,
    ) -> TypeId {
        match self.get_type(ty) {
            Type::Generic(g) => {
                if let Some(replacement) = replacements.get(&g) {
                    *replacement
                } else {
                    ty
                }
            }
            Type::Optional(inner) => {
                let sub_inner = self.substitute_generics(inner, replacements);
                self.session
                    .types
                    .borrow_mut()
                    .intern(Type::Optional(sub_inner))
            }
            Type::Array(inner) => {
                let sub_inner = self.substitute_generics(inner, replacements);
                self.session
                    .types
                    .borrow_mut()
                    .intern(Type::Array(sub_inner))
            }
            Type::Pointer(inner) => {
                let sub_inner = self.substitute_generics(inner, replacements);
                self.session
                    .types
                    .borrow_mut()
                    .intern(Type::Pointer(sub_inner))
            }
            Type::Function(type_params, params, ret) => {
                let sub_params = params
                    .iter()
                    .map(|p| self.substitute_generics(*p, replacements))
                    .collect();
                let sub_ret = self.substitute_generics(ret, replacements);
                self.session.types.borrow_mut().intern(Type::Function(
                    type_params,
                    sub_params,
                    sub_ret,
                ))
            }
            Type::GenericInstance(name, args) => {
                let sub_args = args
                    .iter()
                    .map(|a| self.substitute_generics(*a, replacements))
                    .collect();
                self.session
                    .types
                    .borrow_mut()
                    .intern(Type::GenericInstance(name, sub_args))
            }
            _ => ty,
        }
    }

    pub(crate) fn infer_generics(
        &mut self,
        expected: TypeId,
        actual: TypeId,
        inferred_map: &mut std::collections::HashMap<Symbol, TypeId>,
    ) {
        match (self.get_type(expected), self.get_type(actual)) {
            (Type::Generic(g), _) => {
                if let std::collections::hash_map::Entry::Vacant(e) = inferred_map.entry(g) {
                    e.insert(actual);
                }
            }
            (Type::Optional(e), Type::Optional(a)) => self.infer_generics(e, a, inferred_map),
            (Type::Array(e), Type::Array(a)) => self.infer_generics(e, a, inferred_map),
            (Type::Pointer(e), Type::Pointer(a)) => self.infer_generics(e, a, inferred_map),
            (Type::GenericInstance(e_name, e_args), Type::GenericInstance(a_name, a_args))
                if e_name == a_name =>
            {
                for (e_arg, a_arg) in e_args.iter().zip(a_args.iter()) {
                    self.infer_generics(*e_arg, *a_arg, inferred_map);
                }
            }
            (Type::Enum(e_name, e_params), Type::GenericInstance(a_name, a_args))
            | (Type::Class(e_name, e_params), Type::GenericInstance(a_name, a_args))
                if e_name == a_name =>
            {
                for (e_param, a_arg) in e_params.iter().zip(a_args.iter()) {
                    let gen_id = self
                        .session
                        .types
                        .borrow_mut()
                        .intern(Type::Generic(*e_param));
                    self.infer_generics(gen_id, *a_arg, inferred_map);
                }
            }
            (Type::Function(_, e_params, e_ret), Type::Function(_, a_params, a_ret)) => {
                for (e_param, a_param) in e_params.iter().zip(a_params.iter()) {
                    self.infer_generics(*e_param, *a_param, inferred_map);
                }
                self.infer_generics(e_ret, a_ret, inferred_map);
            }
            _ => {}
        }
    }

    pub(crate) fn type_to_type_expr(&self, ty: TypeId) -> ast::TypeExpr<'a> {
        match self.get_type(ty) {
            Type::Int => ast::TypeExpr::Named(self.session.interner.borrow_mut().intern("Int")),
            Type::Float => ast::TypeExpr::Named(self.session.interner.borrow_mut().intern("Float")),
            Type::Boolean => {
                ast::TypeExpr::Named(self.session.interner.borrow_mut().intern("Boolean"))
            }
            Type::String => {
                ast::TypeExpr::Named(self.session.interner.borrow_mut().intern("String"))
            }
            Type::Instance(name) | Type::Interface(name) => ast::TypeExpr::Named(name),
            Type::GenericInstance(name, args) => ast::TypeExpr::GenericInstance(
                name,
                args.iter().map(|t| self.type_to_type_expr(*t)).collect(),
            ),
            Type::Optional(inner) => {
                ast::TypeExpr::Optional(self.alloc(self.type_to_type_expr(inner)))
            }
            Type::Array(inner) => ast::TypeExpr::Array(self.alloc(self.type_to_type_expr(inner))),
            _ => ast::TypeExpr::Named(self.session.interner.borrow_mut().intern("Any")),
        }
    }

    pub(crate) fn instantiate_generic_class(
        &mut self,
        class_name: Symbol,
        type_params: &[Symbol],
        type_args: &[TypeId],
    ) -> Symbol {
        let type_arg_strings: Vec<String> = type_args
            .iter()
            .map(|t| self.session.format_type(*t))
            .collect();
        let key = generics::SpecializationKey::new(class_name, type_arg_strings);
        let mangled_name_str = key.mangled_name(&self.session.interner.borrow());
        let mangled_name = self.session.interner.borrow_mut().intern(&mangled_name_str);

        if self.spec_registry.get_state(&key) == Some(&generics::SpecializationState::Complete) {
            return mangled_name;
        }
        if self.spec_registry.get_state(&key) == Some(&generics::SpecializationState::Pending) {
            return mangled_name; // Break recursion
        }

        self.spec_registry.mark_pending(key.clone());

        if let Some(generic_stmt) = self.generic_registry.get_class(class_name).cloned() {
            let mut type_arg_exprs = Vec::new();
            for ty in type_args {
                type_arg_exprs.push(self.type_to_type_expr(*ty));
            }

            let param_syms: Vec<session::Symbol> = type_params.to_vec();
            let substitution = self.alloc(generics::TypeSubstitution::new(
                self.arena(),
                &param_syms,
                &type_arg_exprs,
            ));
            let monomorphizer =
                generics::Monomorphizer::new(self.arena(), substitution, mangled_name);

            let concrete_stmt = monomorphizer.monomorphize_stmt(&generic_stmt);
            self.spec_registry.mark_complete(key);

            self.collect_declarations(std::slice::from_ref(&concrete_stmt));

            // Eagerly typecheck the generated class so it's immediately available to the caller
            eprintln!(
                "Eagerly typechecking {}",
                self.session.interner.borrow().lookup(mangled_name)
            );
            let typed_stmt = self.check_stmt(&concrete_stmt);
            eprintln!(
                "Finished eagerly typechecking {}",
                self.session.interner.borrow().lookup(mangled_name)
            );
            self.pending_instantiations.push(typed_stmt);
        }

        mangled_name
    }

    pub(crate) fn instantiate_generic_function(
        &mut self,
        func_name: Symbol,
        type_params: &[Symbol],
        type_args: &[TypeId],
    ) -> Symbol {
        let type_arg_strings: Vec<String> = type_args
            .iter()
            .map(|t| self.session.format_type(*t))
            .collect();
        let key = generics::SpecializationKey::new(func_name, type_arg_strings);
        let mangled_name_str = key.mangled_name(&self.session.interner.borrow());
        let mangled_name = self.session.interner.borrow_mut().intern(&mangled_name_str);

        if self.spec_registry.get_state(&key) == Some(&generics::SpecializationState::Complete) {
            return mangled_name;
        }
        if self.spec_registry.get_state(&key) == Some(&generics::SpecializationState::Pending) {
            return mangled_name; // Break recursion
        }

        self.spec_registry.mark_pending(key.clone());

        if let Some(generic_stmt) = self.generic_registry.get_function(func_name).cloned() {
            let mut type_arg_exprs = Vec::new();
            for ty in type_args {
                type_arg_exprs.push(self.type_to_type_expr(*ty));
            }

            let param_syms: Vec<session::Symbol> = type_params.to_vec();
            let substitution = self.alloc(generics::TypeSubstitution::new(
                self.arena(),
                &param_syms,
                &type_arg_exprs,
            ));
            let monomorphizer =
                generics::Monomorphizer::new(self.arena(), substitution, mangled_name);

            let mut concrete_stmt = monomorphizer.monomorphize_stmt(&generic_stmt);
            if let ast::StmtKind::Func { name, .. } = &mut concrete_stmt.kind {
                *name = mangled_name;
            }

            self.spec_registry.mark_complete(key);

            self.collect_declarations(&[concrete_stmt.clone()]);

            // Eagerly typecheck the generated function so it's immediately available to the caller
            let typed_stmt = self.check_stmt(&concrete_stmt);
            self.pending_instantiations.push(typed_stmt);
        }

        mangled_name
    }

    pub(crate) fn get_assigned_properties_in_init(
        stmt: &TypedStmt,
    ) -> std::collections::HashSet<session::Symbol> {
        let mut assigned = std::collections::HashSet::new();
        match &stmt.kind {
            TypedStmtKind::Block(stmts) => {
                for s in stmts {
                    assigned.extend(Self::get_assigned_properties_in_init(s));
                }
            }
            TypedStmtKind::Expression(expr) => {
                if let TypedExprKind::Set {
                    object,
                    name,
                    value: _,
                } = &expr.kind
                    && let TypedExprKind::SelfRef = &object.kind
                {
                    assigned.insert(*name);
                }
            }
            TypedStmtKind::If {
                then_branch,
                else_branch,
                ..
            } => {
                let then_assigned = Self::get_assigned_properties_in_init(then_branch);
                if let Some(else_b) = else_branch {
                    let else_assigned = Self::get_assigned_properties_in_init(else_b);
                    // Only count if assigned in BOTH branches
                    for prop in then_assigned {
                        if else_assigned.contains(&prop) {
                            assigned.insert(prop);
                        }
                    }
                }
            }
            TypedStmtKind::While { .. } | TypedStmtKind::For { .. } => {
                // Loop bodies might not execute, so we don't count assignments inside them as definite!
            }
            _ => {}
        }
        assigned
    }
}

#[cfg(any())]
mod tests {
    use super::*;
    use ast::Location;
}
