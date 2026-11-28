use super::{ResolvedField, VTableEntry};
use crate::ty::Ty;

use super::TypeChecker;

impl TypeChecker {
    pub fn instantiate_generic(
        &mut self,
        template_name: &str,
        args: &[pace_ast::Type],
    ) -> Result<Ty, String> {
        self.instantiate_generic_named(template_name, args)
            .map(|(ty, _)| ty)
    }

    pub fn instantiate_generic_named(
        &mut self,
        template_name: &str,
        args: &[pace_ast::Type],
    ) -> Result<(Ty, String), String> {
        let template = match self.generic_templates.get(template_name) {
            Some(t) => t.clone(),
            None => return Err(format!("Generic template not found for {}", template_name)),
        };

        let generic_params = match &template {
            pace_hir::Decl::Struct { generic_params, .. } => {
                generic_params.clone().unwrap_or_default()
            }
            pace_hir::Decl::Class { generic_params, .. } => {
                generic_params.clone().unwrap_or_default()
            }
            pace_hir::Decl::Enum { generic_params, .. } => {
                generic_params.clone().unwrap_or_default()
            }
            pace_hir::Decl::Function { generic_params, .. } => {
                generic_params.clone().unwrap_or_default()
            }
            _ => return Err("Unsupported generic declaration".to_string()),
        };

        let mut final_args = Vec::new();
        for (i, (param_name, _trait_bounds, default_ty)) in generic_params.iter().enumerate() {
            if i < args.len() {
                final_args.push(args[i].clone());
            } else if let Some(def_ty) = default_ty {
                final_args.push(def_ty.clone());
            } else {
                return Err(format!(
                    "Missing generic argument for {} and no default provided",
                    param_name
                ));
            }
        }

        if final_args.len() != generic_params.len() {
            return Err(format!(
                "Expected {} generic arguments, got {}",
                generic_params.len(),
                final_args.len()
            ));
        }

        let mut mapping = std::collections::HashMap::new();
        let mut mono_name = template_name.to_string();
        for ((param_name, trait_bounds, _), arg) in
            generic_params.into_iter().zip(final_args.iter())
        {
            mapping.insert(param_name, arg.clone());
            let arg_ty = self.resolve_type(arg)?;

            // Check trait bounds
            if !trait_bounds.is_empty() {
                let hir_id = match &arg_ty {
                    Ty::Struct(id) => Some(*id),
                    Ty::Class(id) => Some(*id),
                    _ => None,
                };

                let impl_traits = hir_id.and_then(|id| self.implemented_traits.get(&id).cloned());
                if let Some(impl_traits) = impl_traits {
                    for bound in &trait_bounds {
                        let bound_ty = self.resolve_type(bound)?;
                        let bound_name = self.display_ty(&bound_ty);
                        if !impl_traits.contains(&bound_name) {
                            return Err(format!(
                                "Type {} does not implement required trait {}",
                                self.display_ty(&arg_ty),
                                bound_name
                            ));
                        }
                    }
                } else if hir_id.is_some() {
                    return Err(format!(
                        "Type {} does not implement any traits, required for generic parameter",
                        self.display_ty(&arg_ty)
                    ));
                } else {
                    return Err(format!(
                        "Type {} cannot satisfy trait bounds",
                        self.display_ty(&arg_ty)
                    ));
                }
            }

            mono_name.push_str(
                &format!("_{:?}", arg_ty)
                    .replace(" ", "")
                    .replace("(", "_")
                    .replace(")", "_")
                    .replace(":", "_"),
            );
        }

        let mut clean_args = Vec::new();
        for arg in &final_args {
            let resolved = self.resolve_type(arg)?;
            clean_args.push(self.display_ty(&resolved));
        }
        let display_name = format!("{}<{}>", template_name, clean_args.join(", "));
        self.display_names.insert(mono_name.clone(), display_name);

        if let Some(&(hir_id, kind)) = self.named_types.get(&mono_name) {
            if kind == 1 {
                return Ok((Ty::Class(hir_id), mono_name));
            } else if kind == 0 {
                return Ok((Ty::Struct(hir_id), mono_name));
            } else {
                return Ok((Ty::Enum(hir_id), mono_name));
            }
        }
        if let Some(fn_ty) = self.methods_env.get(&mono_name).cloned() {
            return Ok((fn_ty, mono_name));
        }

        let mut new_decl = template.clone();

        mapping.insert(
            pace_span::intern(template_name),
            pace_ast::Type::Named(pace_ast::Ident {
                name: pace_span::intern(&mono_name),
                span: pace_span::Span::DUMMY,
            }),
        );

        let short_template_name = template_name.split('_').next_back().unwrap_or(&template_name).to_string();
        mapping.insert(
            pace_span::intern(&short_template_name),
            pace_ast::Type::Named(pace_ast::Ident {
                name: pace_span::intern(&mono_name),
                span: pace_span::Span::DUMMY,
            }),
        );

        match &mut new_decl {
            pace_hir::Decl::Struct {
                id: _,
                name,
                fields,
                static_fields,
                const_fields,
                generic_params,
                methods,
                ..
            }
            | pace_hir::Decl::Class {
                id: _,
                name,
                fields,
                static_fields,
                const_fields,
                generic_params,
                methods,
                .. 
            } => {
                *name = pace_span::intern(&mono_name);
                *generic_params = None;
                for pace_hir::HirFieldDef { ty, .. } in fields.iter_mut() {
                    *ty = substitute_type(ty, &mapping);
                }
                for pace_hir::HirStaticFieldDef { ty, .. } in static_fields.iter_mut() {
                    *ty = substitute_type(ty, &mapping);
                }
                for pace_hir::HirConstFieldDef { ty, .. } in const_fields.iter_mut() {
                    *ty = substitute_type(ty, &mapping);
                }
                for method in methods.iter_mut() {
                    if let pace_hir::Decl::Function {
                        name: m_name,
                        params,
                        return_type,
                        body,
                        ..
                    } = method
                    {
                        let short_template_name = template_name.split('_').next_back().unwrap_or(template_name);
                        let base_method_name = m_name
                            .trim_prefix(&format!("{}_", short_template_name))
                            .to_string();
                        *m_name = pace_span::intern(&format!("{}_{}", mono_name, base_method_name));
                        for (_, _, param_ty) in params.iter_mut() {
                            *param_ty = substitute_type(param_ty, &mapping);
                        }
                        if let Some(ret_ty) = return_type {
                            *ret_ty = substitute_type(ret_ty, &mapping);
                        }
                        substitute_types_in_block(body, &mapping);
                    }
                }
            }
            pace_hir::Decl::Enum {
                id: _,
                name,
                variants,
                generic_params,
                ..
            } => {
                *name = pace_span::intern(&mono_name);
                *generic_params = None;
                for v in variants.iter_mut() {
                    if let Some(fields) = &mut v.fields {
                        for (_, ty) in fields.iter_mut() {
                            *ty = substitute_type(ty, &mapping);
                        }
                    }
                }
            }
            pace_hir::Decl::Function {
                id: _,
                name,
                params,
                return_type,
                generic_params,
                body,
                ..
            } => {
                *name = pace_span::intern(&mono_name);
                *generic_params = None;
                for (_, _, ty) in params.iter_mut() {
                    *ty = substitute_type(ty, &mapping);
                }
                if let Some(rty) = return_type {
                    *rty = substitute_type(rty, &mapping);
                }
                substitute_types_in_block(body, &mapping);
            }
            _ => {}
        }

        crate::reassign::reassign_decl_ids(self, &mut new_decl);

        if let pace_hir::Decl::Function {
            id,
            params,
            return_type,
            ..
        } = &new_decl
        {
            let mut param_tys = Vec::new();
            for (_, _, ty) in params {
                param_tys.push(self.resolve_type(ty)?);
            }
            let ret_ty = if let Some(rty) = return_type {
                self.resolve_type(rty)?
            } else {
                Ty::Void
            };
            let func_ty = Ty::Function(param_tys, Box::new(ret_ty));
            self.env.insert(*id, func_ty.clone());
            self.methods_env.insert(mono_name.clone(), func_ty);
        }

        match &new_decl {
            pace_hir::Decl::Struct {
                id, name, fields, ..
            } => {
                self.named_types.insert(name.to_string(), (*id, 0));
                let mut resolved_fields = Vec::new();
                for pace_hir::HirFieldDef {
                    name: f_name,
                    ty: f_ty,
                    is_mut: is_pub,
                    is_private,
                    ..
                } in fields
                {
                    let ty = self.resolve_type(f_ty)?;
                    resolved_fields.push(ResolvedField {
                        name: f_name.to_string(),
                        ty: ty.clone(),
                        is_mut: *is_pub,
                        is_private: *is_private,
                    });
                }
                self.struct_defs.insert(*id, resolved_fields);
            }
            pace_hir::Decl::Class {
                id,
                name,
                fields,
                methods,
                ..
            } => {
                self.named_types.insert(name.to_string(), (*id, 1));
                let mut resolved_fields = Vec::new();
                for pace_hir::HirFieldDef {
                    name: f_name,
                    ty: f_ty,
                    is_mut: is_pub,
                    is_private,
                    ..
                } in fields
                {
                    let ty = self.resolve_type(f_ty)?;
                    resolved_fields.push(ResolvedField {
                        name: f_name.to_string(),
                        ty: ty.clone(),
                        is_mut: *is_pub,
                        is_private: *is_private,
                    });
                }
                self.class_defs.insert(*id, resolved_fields);

                for method in methods {
                    if let pace_hir::Decl::Function {
                        name: m_name,
                        params,
                        return_type,
                        ..
                    } = method
                    {
                        let mut param_tys = Vec::new();
                        for (_, _, pty) in params {
                            param_tys.push(self.resolve_type(pty)?);
                        }
                        let ret_ty = if let Some(rty) = return_type {
                            self.resolve_type(rty)?
                        } else {
                            Ty::Void
                        };
                        self.methods_env.insert(
                            m_name.to_string(),
                            Ty::Function(param_tys, Box::new(ret_ty)),
                        );
                    }
                }

                let mut vtable = Vec::new();
                for method in methods {
                    if let pace_hir::Decl::Function { name: m_name, .. } = method
                        && let Some(ty) = self.methods_env.get(&m_name.to_string())
                    {
                        let base_name = m_name
                            .trim_prefix(&format!("{}_", name))
                            .to_string();
                        vtable.push(VTableEntry {
                            name: base_name.clone(),
                            ty: ty.clone(),
                            mangled_name: m_name.to_string(),
                        });
                    }
                }
                self.class_vtables.insert(*id, vtable);
            }
            pace_hir::Decl::Enum {
                id, name, variants, ..
            } => {
                self.named_types.insert(name.to_string(), (*id, 2));
                self.enum_defs.insert(*id, variants.clone());
            }
            _ => {}
        }
        self.instantiated_generics.push(new_decl.clone());
        self.check_decl(&new_decl)?;

        match new_decl {
            pace_hir::Decl::Struct { id, .. } => Ok((Ty::Struct(id), mono_name)),
            pace_hir::Decl::Class { id, .. } => Ok((Ty::Class(id), mono_name)),
            pace_hir::Decl::Actor { id, .. } => Ok((Ty::Actor(id), mono_name)),
            pace_hir::Decl::Enum { id, variants, .. } => {
                for v in variants {
                    if let Some(f) = &v.fields {
                        let mut param_tys = Vec::new();
                        for (_, fty) in f {
                            param_tys.push(self.resolve_type(fty)?);
                        }
                        self.env
                            .insert(v.id, Ty::Function(param_tys, Box::new(Ty::Enum(id))));
                    } else {
                        self.env.insert(v.id, Ty::Enum(id));
                    }
                }
                Ok((Ty::Enum(id), mono_name))
            }
            pace_hir::Decl::Function { name, .. } => {
                let fn_ty = self
                    .methods_env
                    .get(&name.to_string())
                    .cloned()
                    .unwrap_or_else(|| Ty::Function(vec![], Box::new(Ty::Void)));
                Ok((fn_ty, mono_name))
            }
            _ => Err("Unsupported declaration".to_string()),
        }
    }

}

pub fn substitute_type(
    ty: &pace_ast::Type,
    mapping: &std::collections::HashMap<pace_span::Symbol, pace_ast::Type>,
) -> pace_ast::Type {
    match ty {
        pace_ast::Type::Named(id) => {
            if let Some(mapped) = mapping.get(&id.name) {
                mapped.clone()
            } else {
                ty.clone()
            }
        }
        pace_ast::Type::Generic(base, args, span) => {
            let sub_base = substitute_type(base, mapping);
            let sub_args = args.iter().map(|a| substitute_type(a, mapping)).collect();
            pace_ast::Type::Generic(Box::new(sub_base), sub_args, *span)
        }
        pace_ast::Type::Optional(inner, span) => {
            pace_ast::Type::Optional(Box::new(substitute_type(inner, mapping)), *span)
        }
        pace_ast::Type::Closure(params, ret, span) => {
            let sub_params = params.iter().map(|p| substitute_type(p, mapping)).collect();
            pace_ast::Type::Closure(sub_params, Box::new(substitute_type(ret, mapping)), *span)
        }
    }
}

fn substitute_types_in_block(
    block: &mut pace_hir::Block,
    mapping: &std::collections::HashMap<pace_span::Symbol, pace_ast::Type>,
) {
    for stmt in &mut block.statements {
        substitute_types_in_stmt(stmt, mapping);
    }
}

fn substitute_types_in_stmt(
    stmt: &mut pace_hir::Stmt,
    mapping: &std::collections::HashMap<pace_span::Symbol, pace_ast::Type>,
) {
    match stmt {
        pace_hir::Stmt::Let { ty, value, .. } | pace_hir::Stmt::Var { ty, value, .. } => {
            if let Some(t) = ty {
                *t = substitute_type(t, mapping);
            }
            if let Some(v) = value {
                substitute_types_in_expr(v, mapping);
            }
        }
        pace_hir::Stmt::ExprStmt(e, _) => substitute_types_in_expr(e, mapping),
        pace_hir::Stmt::Return(Some(e), _) => substitute_types_in_expr(e, mapping),
        pace_hir::Stmt::Return(None, _)
        | pace_hir::Stmt::Break(_)
        | pace_hir::Stmt::Continue(_) => {}
    }
}

fn substitute_types_in_expr(
    expr: &mut pace_hir::Expr,
    mapping: &std::collections::HashMap<pace_span::Symbol, pace_ast::Type>,
) {
    match expr {
        pace_hir::Expr::Ident(_, _, Some(args), _) => {
            for a in args.iter_mut() {
                *a = substitute_type(a, mapping);
            }
        }
        pace_hir::Expr::InterpolatedString(parts, _) => {
            for p in parts.iter_mut() {
                substitute_types_in_expr(p, mapping);
            }
        }
        pace_hir::Expr::Binary { left, right, .. } => {
            substitute_types_in_expr(left, mapping);
            substitute_types_in_expr(right, mapping);
        }
        pace_hir::Expr::MemberAccess { object, .. }
        | pace_hir::Expr::OptionalMemberAccess { object, .. } => {
            substitute_types_in_expr(object, mapping);
        }
        pace_hir::Expr::Call { callee, args, .. } => {
            substitute_types_in_expr(callee, mapping);
            for a in args.iter_mut() {
                substitute_types_in_expr(&mut a.expr, mapping);
            }
        }
        pace_hir::Expr::BuiltinCall(_, args, _) | pace_hir::Expr::ListLiteral(args, _) => {
            for a in args.iter_mut() {
                substitute_types_in_expr(a, mapping);
            }
        }
        pace_hir::Expr::MapLiteral(entries, _) => {
            for (k, v) in entries.iter_mut() {
                substitute_types_in_expr(k, mapping);
                substitute_types_in_expr(v, mapping);
            }
        }
        pace_hir::Expr::If {
            cond,
            then_block,
            else_block,
            ..
        } => {
            substitute_types_in_expr(cond, mapping);
            substitute_types_in_block(then_block, mapping);
            if let Some(eb) = else_block {
                substitute_types_in_block(eb, mapping);
            }
        }
        pace_hir::Expr::While { cond, body, .. } => {
            substitute_types_in_expr(cond, mapping);
            substitute_types_in_block(body, mapping);
        }
        pace_hir::Expr::Assign { target, value, .. }
        | pace_hir::Expr::Index { target, index: value, .. } => {
            substitute_types_in_expr(target, mapping);
            substitute_types_in_expr(value, mapping);
        }
        pace_hir::Expr::Match { subject, arms, .. } => {
            substitute_types_in_expr(subject, mapping);
            for arm in arms.iter_mut() {
                substitute_types_in_expr(&mut arm.body, mapping);
            }
        }
        pace_hir::Expr::Closure {
            params,
            return_type,
            body,
            ..
        } => {
            for p in params.iter_mut() {
                if let Some(t) = &mut p.ty {
                    *t = substitute_type(t, mapping);
                }
            }
            if let Some(rt) = return_type {
                *rt = substitute_type(rt, mapping);
            }
            match body {
                pace_hir::ClosureBody::Expr(e) => substitute_types_in_expr(e, mapping),
                pace_hir::ClosureBody::Block(b) => substitute_types_in_block(b, mapping),
            }
        }
        pace_hir::Expr::Await(inner, _)
        | pace_hir::Expr::Try(inner, _)
        | pace_hir::Expr::Unary { expr: inner, .. } => {
            substitute_types_in_expr(inner, mapping);
        }
        pace_hir::Expr::IntLiteral(_, _)
        | pace_hir::Expr::FloatLiteral(_, _)
        | pace_hir::Expr::BoolLiteral(_, _)
        | pace_hir::Expr::StringLiteral(_, _)
        | pace_hir::Expr::Null(_)
        | pace_hir::Expr::Ident(_, _, None, _)
        | pace_hir::Expr::Super(_) => {}
    }
}
