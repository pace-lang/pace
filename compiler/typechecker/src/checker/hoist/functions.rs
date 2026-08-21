use super::*;
use session::types::Type;

impl<'a> TypeChecker<'a> {
    pub(crate) fn hoist_foreign_func(&mut self, stmt: &Stmt<'a>, name: &session::Symbol, type_params: &[session::Symbol], params: &[(session::Symbol, TypeExpr<'a>)], return_type: &Option<TypeExpr<'a>>) {
        self.env.push_scope();
        for tp in type_params {
            self.env.declare(
                *tp,
                self.session.types.borrow_mut().intern(Type::Generic(*tp)),
            );
        }
        let ret_ty = if let Some(rt) = return_type {
            self.parse_type(rt, stmt.span)
        } else {
            self.session.types.borrow_mut().intern(Type::Void)
        };
        let mut param_types = Vec::new();
        for (_, param_type_str) in params {
            param_types.push(self.parse_type(param_type_str, stmt.span));
        }
        self.env.pop_scope();

        if !type_params.is_empty() {
            self.generic_registry.register_function(*name, stmt.clone());
            return;
        }

        self.env.declare(
            *name,
            self.session.types.borrow_mut().intern(Type::Function(
                type_params.to_vec(),
                param_types.clone(),
                ret_ty,
            )),
        );
    }

    pub(crate) fn hoist_func(&mut self, stmt: &Stmt<'a>, name: &session::Symbol, type_params: &[session::Symbol], params: &[(session::Symbol, TypeExpr<'a>)], return_type: &Option<TypeExpr<'a>>, is_async: bool) {
        self.env.push_scope();
        for tp in type_params {
            self.env.declare(*tp, self.session.types.borrow_mut().intern(Type::Generic(*tp)));
        }

        let mut has_interface = false;
        let mut param_types = Vec::new();
        for (_, param_type_str) in params {
            let pt = self.parse_type(param_type_str, stmt.span);
            param_types.push(pt);
            if let Type::Interface(_, _) = self.get_type(pt) {
                has_interface = true;
            } else if let Type::GenericInstance(base, _) = self.get_type(pt) {
                if self.interfaces.contains_key(&base) || self.generic_registry.get_interface(base).is_some() {
                    has_interface = true;
                }
            }
        }

        if has_interface {
            let mut new_stmt = stmt.clone();
            if let StmtKind::Func { type_params: tps, params: ast_params, .. } = &mut new_stmt.kind {
                let mut gen_idx = tps.len();
                for (i, (_, _)) in params.iter().enumerate() {
                    if let Type::Interface(_, _) = self.get_type(param_types[i]) {
                        let gen_name = format!("__T_{}", gen_idx);
                        let gen_sym = self.session.interner.borrow_mut().intern(&gen_name);
                        tps.push(gen_sym);
                        ast_params[i].1 = ast::TypeExpr::Named(gen_sym);
                        gen_idx += 1;
                    } else if let Type::GenericInstance(base, _) = self.get_type(param_types[i]) {
                        if self.interfaces.contains_key(&base) || self.generic_registry.get_interface(base).is_some() {
                            let gen_name = format!("__T_{}", gen_idx);
                            let gen_sym = self.session.interner.borrow_mut().intern(&gen_name);
                            tps.push(gen_sym);
                            ast_params[i].1 = ast::TypeExpr::Named(gen_sym);
                            gen_idx += 1;
                        }
                    }
                }
            }
            self.generic_registry.register_function(*name, new_stmt);
            self.env.pop_scope();
            return;
        }

        if !type_params.is_empty() {
            self.generic_registry.register_function(*name, stmt.clone());
            self.env.pop_scope();
            return;
        }
        let mut ret_ty = if let Some(rt) = return_type {
            self.parse_type(rt, stmt.span)
        } else {
            self.session.types.borrow_mut().intern(Type::Void)
        };
        if is_async {
            ret_ty = self.session.types.borrow_mut().intern(Type::Task(ret_ty));
        }
        self.env.pop_scope();

        let func_ty = self.session.types.borrow_mut().intern(Type::Function(
            type_params.to_vec(),
            param_types.clone(),
            ret_ty,
        ));
        if let Some(existing) = self.env.resolve(*name) {
            if matches!(
                self.get_type(existing),
                Type::Function(..) | Type::OverloadedFunction(..)
            ) {
                let mut funcs = match self.get_type(existing) {
                    Type::OverloadedFunction(fs) => fs,
                    Type::Function(..) => vec![(*name, existing)],
                    _ => unreachable!(),
                };
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
                let mangled_sym = self.session.interner.borrow_mut().intern(&mangled);
                funcs.push((mangled_sym, func_ty));
                self.env.declare(
                    *name,
                    self.session
                        .types
                        .borrow_mut()
                        .intern(Type::OverloadedFunction(funcs)),
                );
                self.env.declare(mangled_sym, func_ty);
            } else {
                self.env.declare(*name, func_ty);
            }
        } else {
            self.env.declare(*name, func_ty);
        }
    }
}
