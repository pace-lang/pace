use super::ResolvedField;
use pace_hir::Expr;
use std::collections::{HashMap, HashSet};

use crate::ty::Ty;
use pace_errors::{Diagnostic, ErrorCode};

use super::TypeChecker;

impl TypeChecker {
    pub fn check_expr(&mut self, expr: &Expr) -> Result<Ty, String> {
        let res = self.check_expr_inner(expr);
        if let Ok(ty) = &res {
            let span = expr.span();
            let display = self.display_ty(ty);
            self.hover_info.insert(span, format!("```pace\n{}\n```", display));
        }
        res
    }

    fn check_expr_inner(&mut self, expr: &Expr) -> Result<Ty, String> {
        match expr {
            Expr::IntLiteral(..) => Ok(Ty::Int),
            Expr::FloatLiteral(..) => Ok(Ty::Float),
            Expr::BoolLiteral(..) => Ok(Ty::Bool),
            Expr::StringLiteral(..) => Ok(Ty::String),
            Expr::InterpolatedString(exprs, _) => {
                for expr in exprs {
                    self.check_expr(expr)?;
                }
                Ok(Ty::String)
            }
            Expr::Super(span) => {
                // Find "self" in current environment
                for (id, ty) in &self.env {
                    if let Some((_, name, _)) = self
                        .declared_bindings
                        .iter()
                        .find(|(b_id, _, _)| b_id == id)
                        && name == "self"
                        && let Ty::Class(class_id) = ty
                    {
                        if let Some(&parent_id) = self.class_parents.get(class_id) {
                            return Ok(Ty::Class(parent_id));
                        } else {
                            self.reporter.report(
                                pace_errors::Diagnostic::error(
                                    "Cannot use 'super' in a class with no parent",
                                )
                                .with_span(*span),
                            );
                            return Err("Cannot use 'super' in a class with no parent".to_string());
                        }
                    }
                }
                self.reporter.report(
                    pace_errors::Diagnostic::error("Cannot use 'super' outside of a class method")
                        .with_span(*span),
                );
                Err("Cannot use 'super' outside of a class method".to_string())
            }
            Expr::Null(_span) => {
                if let Some(expected) = &self.current_expected_ty
                    && let Ty::Optional(_) = expected
                {
                    return Ok(expected.clone());
                }
                // Default to Option<Void> if context is unknown
                Ok(Ty::Optional(Box::new(Ty::Void)))
            }
            Expr::Ident(id, name, generic_args, span) => {
                self.used_bindings.insert(*id);
                if !self.initialized_bindings.contains(id)
                    && let Some((_, name, _)) = self
                        .declared_bindings
                        .iter()
                        .find(|(d_id, _, _)| d_id == id)
                {
                    self.reporter.report(
                        Diagnostic::error(format!(
                            "Non-nullable variable '{}' must be assigned before it can be used",
                            name
                        ))
                        .with_span(*span)
                        .with_code(ErrorCode::UninitializedVariable),
                    );
                }
                let mangled_name = if let Some(curr) = &self.current_module {
                    if let Some(scope) = self.module_scopes.get(curr) {
                        scope
                            .visible_symbols
                            .get(&name.to_string())
                            .cloned()
                            .unwrap_or_else(|| name.to_string())
                    } else {
                        name.to_string()
                    }
                } else {
                    name.to_string()
                };

                if generic_args.is_some()
                    && (self.generic_templates.contains_key(&mangled_name)
                        || self.generic_templates.contains_key(name.as_str()))
                {
                    let gname = if self.generic_templates.contains_key(&mangled_name) {
                        &mangled_name
                    } else {
                        name.as_str()
                    };
                    if let Some(args) = generic_args {
                        let (inst_ty, mono_name) = self.instantiate_generic_named(gname, args)?;
                        self.env.insert(*id, inst_ty.clone());
                        self.local_types.insert(*id, inst_ty.clone());
                        self.resolved_global_names.insert(*id, mono_name);
                        return Ok(inst_ty);
                    }
                }

                if let Some(ty) = self.env.get(id).cloned() {
                    self.symbol_references.entry(*id).or_default().push(*span);
                    if let Some((_, _, decl_span)) = self.declared_bindings.iter().find(|(d_id, _, _)| d_id == id) {
                        self.definition_info.insert(*span, *decl_span);
                    }
                    Ok(ty)
                } else if let Some(ty) = self.global_functions.get(&mangled_name).cloned() {
                    self.used_bindings_by_name.insert(name.to_string());
                    self.resolved_global_names.insert(*id, mangled_name.clone());
                    self.local_types.insert(*id, ty.clone());
                    if let Some(def_span) = self.global_spans.get(&mangled_name) {
                        self.definition_info.insert(*span, *def_span);
                    }
                    Ok(ty)
                } else if let Some(ty) = self.global_vars.get(&mangled_name).cloned() {
                    self.used_bindings_by_name.insert(name.to_string());
                    self.env.insert(*id, ty.clone());
                    self.local_types.insert(*id, ty.clone());
                    self.resolved_global_names.insert(*id, mangled_name.clone());
                    if let Some(def_span) = self.global_spans.get(&mangled_name) {
                        self.definition_info.insert(*span, *def_span);
                    }
                    Ok(ty)
                } else if let Some(&(hir_id, kind)) = self.named_types.get(&mangled_name) {
                    let ty = if kind == 0 {
                        Ty::Struct(hir_id)
                    } else if kind == 1 {
                        Ty::Class(hir_id)
                    } else if kind == 3 {
                        Ty::Actor(hir_id)
                    } else {
                        Ty::Enum(hir_id)
                    };
                    self.env.insert(*id, ty.clone());
                    self.local_types.insert(*id, ty.clone());
                    self.resolved_global_names.insert(*id, mangled_name.clone());
                    if let Some(def_span) = self.global_spans.get(&mangled_name) {
                        self.definition_info.insert(*span, *def_span);
                    }
                    Ok(ty)
                } else if self.generic_templates.contains_key(&mangled_name)
                    || self.generic_templates.contains_key(name.as_str())
                {
                    let gname = if self.generic_templates.contains_key(&mangled_name) {
                        &mangled_name
                    } else {
                        name.as_str()
                    };
                    if let Some(args) = generic_args {
                        let (inst_ty, mono_name) = self.instantiate_generic_named(gname, args)?;
                        self.env.insert(*id, inst_ty.clone());
                        self.local_types.insert(*id, inst_ty.clone());
                        self.resolved_global_names.insert(*id, mono_name);
                        return Ok(inst_ty);
                    }
                    if let Some(expected) = &self.current_expected_ty {
                        match expected {
                            Ty::Struct(hir_id) | Ty::Class(hir_id) => {
                                if let Some(constructor_ty) = self.env.get(hir_id).cloned() {
                                    Ok(constructor_ty)
                                } else {
                                    Err(format!(
                                        "Constructor not found for instantiated generic '{}'",
                                        name
                                    ))
                                }
                            }
                            _ => Ok(expected.clone()),
                        }
                    } else {
                        Err(format!("Cannot infer type for generic template '{}'", name))
                    }
                } else if name == "List" {
                    if let Some(args) = generic_args && args.len() == 1 {
                        let inner = self.resolve_type(&args[0])?;
                        let inst_ty = Ty::List(Box::new(inner));
                        self.local_types.insert(*id, inst_ty.clone());
                        return Ok(inst_ty);
                    }
                    if let Some(Ty::List(inner)) = &self.current_expected_ty {
                        let inst_ty = Ty::List(inner.clone());
                        self.local_types.insert(*id, inst_ty.clone());
                        return Ok(inst_ty);
                    }
                    return Err("List requires a type parameter, e.g. List<int>".to_string());
                } else if name == "Map" {
                    if let Some(args) = generic_args && args.len() == 2 {
                        let k = self.resolve_type(&args[0])?;
                        let v = self.resolve_type(&args[1])?;
                        let inst_ty = Ty::Map(Box::new(k), Box::new(v));
                        self.local_types.insert(*id, inst_ty.clone());
                        return Ok(inst_ty);
                    }
                    if let Some(Ty::Map(k, v)) = &self.current_expected_ty {
                        let inst_ty = Ty::Map(k.clone(), v.clone());
                        self.local_types.insert(*id, inst_ty.clone());
                        return Ok(inst_ty);
                    }
                    return Err("Map requires two type parameters, e.g. Map<int, int>".to_string());
                } else if name == "Set" {
                    if let Some(args) = generic_args && args.len() == 1 {
                        let inner = self.resolve_type(&args[0])?;
                        let inst_ty = Ty::Set(Box::new(inner));
                        self.local_types.insert(*id, inst_ty.clone());
                        return Ok(inst_ty);
                    }
                    if let Some(Ty::Set(inner)) = &self.current_expected_ty {
                        let inst_ty = Ty::Set(inner.clone());
                        self.local_types.insert(*id, inst_ty.clone());
                        return Ok(inst_ty);
                    }
                    return Err("Set requires a type parameter, e.g. Set<int>".to_string());
                } else if name == "StringBuilder" {
                    let inst_ty = Ty::StringBuilder;
                    self.env.insert(*id, inst_ty.clone());
                    self.local_types.insert(*id, inst_ty.clone());
                    return Ok(inst_ty);
                } else {
                    let err_msg = format!("Cannot find value '{}' in this scope", name);
                    self.reporter
                        .report(Diagnostic::error(&err_msg).with_span(*span));
                    Err(err_msg)
                }
            }
            Expr::Unary { op, expr, .. } => {
                let inner_ty = self.check_expr(expr)?;
                match op {
                    pace_ast::UnaryOp::Neg => {
                        if inner_ty == Ty::Int || inner_ty == Ty::Float {
                            Ok(inner_ty)
                        } else {
                            Err(format!(
                                "Unary '-' requires int or float, found {}",
                                self.display_ty(&inner_ty)
                            ))
                        }
                    }
                    pace_ast::UnaryOp::Not => {
                        if inner_ty == Ty::Bool || inner_ty == Ty::Int {
                            Ok(Ty::Bool)
                        } else {
                            Err(format!(
                                "Unary '!' requires bool, found {}",
                                self.display_ty(&inner_ty)
                            ))
                        }
                    }
                }
            }
            Expr::Binary {
                left, op, right, ..
            } => {
                let left_ty = self.check_expr(left)?;
                let right_ty = self.check_expr(right)?;

                match op {
                    pace_ast::BinaryOp::Add
                    | pace_ast::BinaryOp::Sub
                    | pace_ast::BinaryOp::Mul
                    | pace_ast::BinaryOp::Div
                    | pace_ast::BinaryOp::Mod => {
                        if left_ty == Ty::Int && right_ty == Ty::Int {
                            Ok(Ty::Int)
                        } else if left_ty == Ty::Float && right_ty == Ty::Float {
                            Ok(Ty::Float)
                        } else {
                            Err(format!(
                                "Type mismatch in binary operation: {} and {}",
                                self.display_ty(&left_ty),
                                self.display_ty(&right_ty)
                            ))
                        }
                    }
                    pace_ast::BinaryOp::EqEq
                    | pace_ast::BinaryOp::NotEq
                    | pace_ast::BinaryOp::Gt
                    | pace_ast::BinaryOp::Lt
                    | pace_ast::BinaryOp::GtEq
                    | pace_ast::BinaryOp::LtEq => Ok(Ty::Bool),
                    pace_ast::BinaryOp::And | pace_ast::BinaryOp::Or => Ok(Ty::Bool),
                    pace_ast::BinaryOp::NullCoalesce => {
                        if let Ty::Optional(inner) = &left_ty {
                            if **inner == right_ty {
                                Ok(right_ty.clone())
                            } else {
                                Err(format!(
                                    "Type mismatch in '??': left is {}, but right is {}",
                                    self.display_ty(&left_ty),
                                    self.display_ty(&right_ty)
                                ))
                            }
                        } else {
                            Err(format!(
                                "Left side of '??' must be an optional type, found {}",
                                self.display_ty(&left_ty)
                            ))
                        }
                    }
                }
            }
            Expr::OptionalMemberAccess { object, member, .. } => {
                let obj_ty = self.check_expr(object)?;
                if let Ty::Optional(inner) = obj_ty {
                    if let Ty::Struct(hir_id) | Ty::Class(hir_id) = *inner {
                        if let Ty::Struct(_) = *inner {
                            if let Some(fields) = self.struct_defs.get(&hir_id) {
                                for ResolvedField {
                                    name: fname,
                                    ty: fty,
                                    ..
                                } in fields
                                {
                                    if fname == member {
                                        return Ok(if matches!(fty, Ty::Optional(_)) {
                                            fty.clone()
                                        } else {
                                            Ty::Optional(Box::new(fty.clone()))
                                        });
                                    }
                                }
                            }
                        } else {
                            if let Some(fields) = self.class_defs.get(&hir_id) {
                                for ResolvedField {
                                    name: fname,
                                    ty: fty,
                                    ..
                                } in fields
                                {
                                    if fname == member {
                                        return Ok(if matches!(fty, Ty::Optional(_)) {
                                            fty.clone()
                                        } else {
                                            Ty::Optional(Box::new(fty.clone()))
                                        });
                                    }
                                }
                            }
                        }
                    }

                    Err(format!("Cannot optional chain on {}", self.display_ty(&inner)))
                } else {
                    Err(format!(
                        "Left side of '?.' must be an optional type, found {}",
                        self.display_ty(&obj_ty)
                    ))
                }
            }
            Expr::MemberAccess {
                object,
                member,
                span,
            } => {
                // Intercept module aliases (e.g. `d.hello` where `d` is an alias for `demo_lib`)
                if let Expr::Ident(id, name, _, _) = &**object
                    && let Some(curr) = &self.current_module
                    && let Some(scope) = self.module_scopes.get(curr)
                    && let Some(target_module) = scope.aliases.get(&name.to_string())
                {
                    // It IS a module alias access!
                    if let Some(target_scope) = self.module_scopes.get(target_module) {
                        if target_scope.private_symbols.contains(&member.to_string()) {
                            let err = format!(
                                "Symbol '{}' is private in module '{}'",
                                member, target_module
                            );
                            self.reporter.report(Diagnostic::error(&err).with_span(*span));
                            return Err(err);
                        }
                        if let Some(mangled_name) =
                            target_scope.visible_symbols.get(&member.to_string())
                        {
                        // The object is a module reference. Record the target module so MirBuilder can use it.
                        self.resolved_global_names
                            .insert(*id, target_module.to_string());
                        self.used_bindings_by_name.insert(member.to_string());

                        if let Some(ty) = self.global_functions.get(mangled_name).cloned() {
                            if let Some(def_span) = self.global_spans.get(mangled_name) {
                                self.definition_info.insert(*span, *def_span);
                            }
                            return Ok(ty);
                        } else if let Some(ty) = self.global_vars.get(mangled_name).cloned() {
                            if let Some(def_span) = self.global_spans.get(mangled_name) {
                                self.definition_info.insert(*span, *def_span);
                            }
                            return Ok(ty);
                        } else if let Some(&(hir_id, kind)) = self.named_types.get(mangled_name) {
                            let ty = if kind == 0 {
                                Ty::Struct(hir_id)
                            } else if kind == 1 {
                                Ty::Class(hir_id)
                            } else {
                                Ty::Enum(hir_id)
                            };
                            if let Some(def_span) = self.global_spans.get(mangled_name) {
                                self.definition_info.insert(*span, *def_span);
                            }
                            return Ok(ty);
                        }
                    }
                }
                }

                let obj_ty = self.check_expr(object)?;
                match obj_ty {
                    Ty::Struct(hir_id) => {
                        let mut struct_name = "";
                        for (name, &(nid, _)) in &self.named_types {
                            if nid == hir_id {
                                struct_name = name;
                                break;
                            }
                        }

                        let fields = self
                            .struct_defs
                            .get(&hir_id)
                            .ok_or("Struct definition not found")?;
                        for ResolvedField {
                            name: fname,
                            ty: fty,
                            is_private,
                            ..
                        } in fields
                        {
                            if fname == member {
                                if *is_private {
                                    let mut can_access = false;
                                    if let Some(current_fn) = &self.current_fn_name
                                        && current_fn.starts_with(&format!("{}_", struct_name))
                                    {
                                        can_access = true;
                                    }
                                    if !can_access {
                                        return Err(format!(
                                            "Field '{}' of '{}' is private",
                                            member, struct_name
                                        ));
                                    }
                                }
                                self.used_bindings_by_name.insert(fname.clone());
                                return Ok(fty.clone());
                            }
                        }

                        // Check if it's a static field
                        if let Some(sfty) = self
                            .static_fields_env
                            .get(&format!("{}_{}", struct_name, member))
                        {
                            return Ok(sfty.clone());
                        }

                        // Check if it's a method
                        let method_name = format!("{}_{}", struct_name, member);
                        if let Some(mty) = self.methods_env.get(&method_name) {
                            self.used_bindings_by_name.insert(method_name.clone());
                            let short_struct = struct_name.split('_').next_back().unwrap_or(struct_name);
                            self.used_bindings_by_name
                                .insert(format!("{}_{}", short_struct, member));
                            if let Some(def_span) = self.global_spans.get(&method_name) {
                                self.definition_info.insert(*span, *def_span);
                            }
                            return Ok(mty.clone());
                        }

                        Err(format!("Struct has no member '{}'", member))
                    }
                    Ty::Class(hir_id) | Ty::Actor(hir_id) => {
                        let is_actor = matches!(obj_ty, Ty::Actor(_));
                        let mut class_name = "";
                        for (name, &(nid, _)) in &self.named_types {
                            if nid == hir_id {
                                class_name = name;
                                break;
                            }
                        }

                        let fields = self
                            .class_defs
                            .get(&hir_id)
                            .ok_or("Class/Actor definition not found")?;
                        for ResolvedField {
                            name: fname,
                            ty: fty,
                            is_private,
                            ..
                        } in fields
                        {
                            if fname == member {
                                if *is_private || is_actor {
                                    let mut can_access = false;
                                    if let Some(current_fn) = &self.current_fn_name {
                                        if current_fn.starts_with(&format!("{}_", class_name)) {
                                            can_access = true;
                                        }
                                    }
                                    if !can_access {
                                        return Err(format!(
                                            "Field '{}' of '{}' is private",
                                            member, class_name
                                        ));
                                    }
                                }
                                self.used_bindings_by_name.insert(fname.clone());
                                return Ok(fty.clone());
                            }
                        }

                        // Check if it's a static field
                        if let Some(sfty) = self
                            .static_fields_env
                            .get(&format!("{}_{}", class_name, member))
                        {
                            return Ok(sfty.clone());
                        }

                        // Check if it's a method (including inherited methods)
                        let mut curr_class_id = Some(hir_id);
                        while let Some(cid) = curr_class_id {
                            let mut cname = "";
                            for (name, &(nid, _)) in &self.named_types {
                                if nid == cid {
                                    cname = name;
                                    break;
                                }
                            }
                            let method_name = format!("{}_{}", cname, member);
                            if let Some(mty) = self.methods_env.get(&method_name) {
                                self.used_bindings_by_name.insert(method_name.clone());
                                let short_class = cname.split('_').next_back().unwrap_or(cname);
                                self.used_bindings_by_name
                                    .insert(format!("{}_{}", short_class, member));
                                if let Some(def_span) = self.global_spans.get(&method_name) {
                                    self.definition_info.insert(*span, *def_span);
                                }
                                return Ok(mty.clone());
                            }
                            curr_class_id = self.class_parents.get(&cid).copied();
                        }

                        Err(format!("Class/Actor has no member '{}'", member))
                    }
                    Ty::Enum(hir_id) => {
                        let variants = self
                            .enum_defs
                            .get(&hir_id)
                            .ok_or("Enum definition not found")?
                            .clone();
                        for v in variants {
                            if &v.name == member {
                                if let Some(fields) = &v.fields {
                                    let mut param_tys = Vec::new();
                                    for (_, fty) in fields {
                                        param_tys.push(self.resolve_type(fty)?);
                                    }
                                    return Ok(Ty::Function(param_tys, Box::new(Ty::Enum(hir_id))));
                                } else {
                                    return Ok(Ty::Enum(hir_id));
                                }
                            }
                        }
                        Err(format!("Enum has no variant '{}'", member))
                    }
                    Ty::List(elem) => {
                        match member.as_str() {
                            "__iter" => Ok(Ty::Function(vec![], Box::new(Ty::List(elem.clone())))),
                            "len" => Ok(Ty::Int),
                            "isEmpty" | "isNotEmpty" => Ok(Ty::Bool),
                            "first" | "last" => Ok(Ty::Optional(elem.clone())),
                            "push" | "add" => Ok(Ty::Function(vec![*elem.clone()], Box::new(Ty::Void))),
                            "pop" => Ok(Ty::Function(vec![], Box::new(Ty::Optional(elem.clone())))),
                            "get" => Ok(Ty::Function(vec![Ty::Int], elem.clone())),
                            "set" => Ok(Ty::Function(vec![Ty::Int, *elem.clone()], Box::new(Ty::Void))),
                            "insert" => Ok(Ty::Function(vec![Ty::Int, *elem.clone()], Box::new(Ty::Void))),
                            "removeAt" => Ok(Ty::Function(vec![Ty::Int], elem.clone())),
                            "contains" => Ok(Ty::Function(vec![*elem.clone()], Box::new(Ty::Bool))),
                            "indexOf" => Ok(Ty::Function(vec![*elem.clone()], Box::new(Ty::Int))),
                            "slice" => Ok(Ty::Function(vec![Ty::Int, Ty::Int], Box::new(Ty::List(elem.clone())))),
                            "reversed" => Ok(Ty::Function(vec![], Box::new(Ty::List(elem.clone())))),
                            "join" => Ok(Ty::Function(vec![Ty::String], Box::new(Ty::String))),
                            "clear" => Ok(Ty::Function(vec![], Box::new(Ty::Void))),
                            "forEach" => Ok(Ty::Function(
                                vec![Ty::Closure(vec![*elem.clone()], Box::new(Ty::Void))],
                                Box::new(Ty::Void),
                            )),
                            "filter" => Ok(Ty::Function(
                                vec![Ty::Closure(vec![*elem.clone()], Box::new(Ty::Bool))],
                                Box::new(Ty::List(elem.clone())),
                            )),
                            "find" => Ok(Ty::Function(
                                vec![Ty::Closure(vec![*elem.clone()], Box::new(Ty::Bool))],
                                Box::new(Ty::Optional(elem.clone())),
                            )),
                            "any" | "every" => Ok(Ty::Function(
                                vec![Ty::Closure(vec![*elem.clone()], Box::new(Ty::Bool))],
                                Box::new(Ty::Bool),
                            )),
                            _ => Err(format!("List has no member '{}'", member)),
                        }
                    }
                    Ty::Map(k, v) => {
                        match member.as_str() {
                            "__iter" => Ok(Ty::Function(vec![], Box::new(Ty::List(k.clone())))),
                            "len" => Ok(Ty::Int),
                            "isEmpty" | "isNotEmpty" => Ok(Ty::Bool),
                            "set" => Ok(Ty::Function(vec![*k.clone(), *v.clone()], Box::new(Ty::Void))),
                            "get" => Ok(Ty::Function(vec![*k.clone()], Box::new(Ty::Optional(v.clone())))),
                            "getOrDefault" => Ok(Ty::Function(vec![*k.clone(), *v.clone()], v.clone())),
                            "has" | "containsKey" => Ok(Ty::Function(vec![*k.clone()], Box::new(Ty::Bool))),
                            "remove" => Ok(Ty::Function(vec![*k.clone()], Box::new(Ty::Bool))),
                            "clear" => Ok(Ty::Function(vec![], Box::new(Ty::Void))),
                            "keys" => Ok(Ty::Function(vec![], Box::new(Ty::List(k.clone())))),
                            "values" => Ok(Ty::Function(vec![], Box::new(Ty::List(v.clone())))),
                            "forEach" => Ok(Ty::Function(
                                vec![Ty::Closure(vec![*k.clone(), *v.clone()], Box::new(Ty::Void))],
                                Box::new(Ty::Void),
                            )),
                            _ => Err(format!("Map has no member '{}'", member)),
                        }
                    }
                    Ty::Set(elem) => {
                        let set_ty = Ty::Set(elem.clone());
                        match member.as_str() {
                            "__iter" => Ok(Ty::Function(vec![], Box::new(Ty::List(elem.clone())))),
                            "len" => Ok(Ty::Int),
                            "isEmpty" | "isNotEmpty" => Ok(Ty::Bool),
                            "add" => Ok(Ty::Function(vec![*elem.clone()], Box::new(Ty::Bool))),
                            "has" | "contains" => Ok(Ty::Function(vec![*elem.clone()], Box::new(Ty::Bool))),
                            "remove" => Ok(Ty::Function(vec![*elem.clone()], Box::new(Ty::Bool))),
                            "clear" => Ok(Ty::Function(vec![], Box::new(Ty::Void))),
                            "toList" => Ok(Ty::Function(vec![], Box::new(Ty::List(elem.clone())))),
                            "union" | "intersection" | "difference" => {
                                Ok(Ty::Function(vec![set_ty.clone()], Box::new(set_ty)))
                            }
                            _ => Err(format!("Set has no member '{}'", member)),
                        }
                    }
                    Ty::String => match member.as_str() {
                        "len" => Ok(Ty::Int),
                        "isEmpty" | "isNotEmpty" => Ok(Ty::Bool),
                        "contains" | "startsWith" | "endsWith" => {
                            Ok(Ty::Function(vec![Ty::String], Box::new(Ty::Bool)))
                        }
                        "indexOf" => Ok(Ty::Function(vec![Ty::String], Box::new(Ty::Int))),
                        "trim" | "toLower" | "toUpper" => {
                            Ok(Ty::Function(vec![], Box::new(Ty::String)))
                        }
                        "slice" => Ok(Ty::Function(vec![Ty::Int, Ty::Int], Box::new(Ty::String))),
                        "replace" => Ok(Ty::Function(
                            vec![Ty::String, Ty::String],
                            Box::new(Ty::String),
                        )),
                        "split" => Ok(Ty::Function(
                            vec![Ty::String],
                            Box::new(Ty::List(Box::new(Ty::String))),
                        )),
                        _ => Err(format!("string has no member '{}'", member)),
                    },
                    Ty::StringBuilder => match member.as_str() {
                        "len" => Ok(Ty::Int),
                        "isEmpty" | "isNotEmpty" => Ok(Ty::Bool),
                        "append" => Ok(Ty::Function(vec![Ty::String], Box::new(Ty::Void))),
                        "toString" => Ok(Ty::Function(vec![], Box::new(Ty::String))),
                        "clear" => Ok(Ty::Function(vec![], Box::new(Ty::Void))),
                        _ => Err(format!("StringBuilder has no member '{}'", member)),
                    },
                    Ty::Result(_, _) => {
                        match member.as_str() {
                            "isOk" | "isError" => Ok(Ty::Function(vec![], Box::new(Ty::Bool))),
                            _ => Err(format!("Result has no member '{}'", member)),
                        }
                    }
                    _ => Err(format!(
                        "Cannot access member '{}' on type {}",
                        member,
                        self.display_ty(&obj_ty)
                    )),
                }
            }
            Expr::Call { callee, args, span } => {
                let mut generic_instantiation = None;
                if let Expr::Ident(callee_id, name, generic_args, _) = &**callee {
                    let mangled_name = if let Some(curr) = &self.current_module {
                        if let Some(scope) = self.module_scopes.get(curr) {
                            scope
                                .visible_symbols
                                .get(&name.to_string())
                                .cloned()
                                .unwrap_or_else(|| name.to_string())
                        } else {
                            name.to_string()
                        }
                    } else {
                        name.to_string()
                    };
                    let gname = if self.generic_templates.contains_key(&mangled_name) {
                        Some(mangled_name.as_str())
                    } else if self.generic_templates.contains_key(name.as_str()) {
                        Some(name.as_str())
                    } else {
                        None
                    };
                    if let (Some(explicit_args), Some(gname)) = (generic_args, gname) {
                        if let Ok((func_ty, mono_name)) =
                            self.instantiate_generic_named(gname, explicit_args)
                        {
                            self.resolved_global_names.insert(*callee_id, mono_name);
                            generic_instantiation = Some(func_ty);
                        }
                    } else if self.generic_templates.contains_key(&name.to_string()) {
                        let template = self
                            .generic_templates
                            .get(&name.to_string())
                            .unwrap()
                            .clone();
                        if let pace_hir::Decl::Function {
                            generic_params,
                            params,
                            ..
                        } = template
                        {
                            let generic_params = generic_params.unwrap_or_default();
                            let mut inferred_args = std::collections::HashMap::new();
                            for (i, pace_hir::HirCallArg { expr: arg_expr, .. }) in
                                args.iter().enumerate()
                            {
                                if i < params.len() {
                                    let arg_ty = self.check_expr(arg_expr)?;
                                    let (_, _, param_ty) = &params[i];
                                    if let pace_ast::Type::Named(ident) = param_ty
                                        && generic_params.iter().any(|(p, _, _)| p == &ident.name)
                                    {
                                        inferred_args.insert(ident.name.to_string(), arg_ty);
                                    }
                                }
                            }

                            let mut ast_args = Vec::new();
                            for (param_name, _, _) in &generic_params {
                                if let Some(ty) = inferred_args.get(&param_name.to_string()) {
                                    let ty_str = format!("{:?}", ty)
                                        .replace(" ", "")
                                        .replace("(", "_")
                                        .replace(")", "_")
                                        .replace(":", "_");
                                    ast_args.push(pace_ast::Type::Named(pace_ast::Ident {
                                        name: pace_span::intern(&ty_str),
                                        span: *span,
                                    }));
                                }
                            }

                            if ast_args.len() == generic_params.len()
                                && let Ok((func_ty, mono_name)) =
                                    self.instantiate_generic_named(name, &ast_args)
                            {
                                self.resolved_global_names.insert(*callee_id, mono_name);
                                generic_instantiation = Some(func_ty);
                            }
                        }
                    }
                } else if let Expr::MemberAccess { object, member, .. } = &**callee
                    && member == "len"
                    && args.is_empty()
                {
                    let obj_ty = self.check_expr(object)?;
                    if matches!(
                        obj_ty,
                        Ty::List(_) | Ty::Map(_, _) | Ty::Set(_) | Ty::String | Ty::StringBuilder
                    ) {
                        return Ok(Ty::Int);
                    }
                } else if let Expr::MemberAccess { object, member, .. } = &**callee
                    && (member == "map" || member == "reduce")
                {
                    let obj_ty = self.check_expr(object)?;
                    if let Ty::List(_) = obj_ty {
                        if member == "map" && args.len() == 1 {
                            let fn_ty = self.check_expr(&args[0].expr)?;
                            if let Ty::Function(_, ret) | Ty::Closure(_, ret) = fn_ty {
                                return Ok(Ty::List(ret));
                            }
                        } else if member == "reduce" && args.len() == 2 {
                            let init_ty = self.check_expr(&args[0].expr)?;
                            let fn_ty = self.check_expr(&args[1].expr)?;
                            if let Ty::Function(_, ret) | Ty::Closure(_, ret) = fn_ty {
                                return Ok(*ret);
                            }
                            return Ok(init_ty);
                        }
                    }
                }

                let callee_ty = if let Some(ty) = generic_instantiation {
                    ty
                } else {
                    self.check_expr(callee)?
                };

                if let Ty::Struct(id) = callee_ty {
                    let mut struct_name = "";
                    for (name, &(nid, _)) in &self.named_types {
                        if nid == id {
                            struct_name = name;
                            break;
                        }
                    }
                    let init_name = format!("{}_init", struct_name);

                    if let Some(Ty::Function(param_tys, _)) =
                        self.methods_env.get(&init_name).cloned()
                    {
                        if args.len() != param_tys.len() - 1 {
                            self.reporter.report(
                                Diagnostic::error(format!(
                                    "{} takes {} arguments, got {}",
                                    init_name,
                                    param_tys.len() - 1,
                                    args.len()
                                ))
                                .with_span(*span)
                                .with_code(ErrorCode::ArityMismatch),
                            );
                            return Err(format!(
                                "{} takes {} arguments, got {}",
                                init_name,
                                param_tys.len() - 1,
                                args.len()
                            ));
                        }
                        for (i, pace_hir::HirCallArg { expr: fexpr, .. }) in args.iter().enumerate()
                        {
                            let fty = self.check_expr(fexpr)?;
                            if fty != param_tys[i + 1] {
                                self.reporter.report(
                                    Diagnostic::error(format!(
                                        "Argument {} expects type {}, got {}",
                                        i,
                                        self.display_ty(&param_tys[i + 1]),
                                        self.display_ty(&fty)
                                    ))
                                    .with_span(*span)
                                    .with_code(ErrorCode::TypeMismatch),
                                );
                                return Err(format!(
                                    "Argument {} expects type {}, got {}",
                                    i,
                                    self.display_ty(&param_tys[i + 1]),
                                    self.display_ty(&fty)
                                ));
                            }
                        }
                        return Ok(Ty::Struct(id));
                    }

                    let def_fields = match self.struct_defs.get(&id) {
                        Some(fields) => fields.clone(),
                        None => {
                            let type_name = if struct_name.is_empty() {
                                format!("{:?}", id)
                            } else {
                                struct_name.to_string()
                            };
                            self.reporter.report(
                                Diagnostic::error(format!("Undefined struct: '{}'", type_name))
                                    .with_span(*span)
                                    .with_code(ErrorCode::UndefinedType),
                            );
                            return Err(format!("Undefined struct: '{}'", type_name));
                        }
                    };
                    let mut def_map: std::collections::HashMap<_, _> = def_fields
                        .into_iter()
                        .map(|f| (f.name.clone(), f.ty.clone()))
                        .collect();
                    for pace_hir::HirCallArg { label, expr: fexpr } in args {
                        let prev_expected = self.current_expected_ty.take();
                        self.current_expected_ty = label
                            .as_ref()
                            .and_then(|fname| def_map.get(fname.as_str()).cloned());
                        let fty = self.check_expr(fexpr)?;
                        self.current_expected_ty = prev_expected;
                        if let Some(fname) = label {
                            if let Some(expected_ty) = def_map.remove(fname.as_str()) {
                                if !self.is_assignable(&expected_ty, &fty) {
                                    self.reporter.report(
                                        Diagnostic::error(format!(
                                            "Field '{}' expects type {}, got {}",
                                            fname,
                                            self.display_ty(&expected_ty),
                                            self.display_ty(&fty)
                                        ))
                                        .with_span(*span)
                                        .with_code(ErrorCode::TypeMismatch),
                                    );
                                    return Err(format!(
                                        "Field '{}' expects type {}, got {}",
                                        fname,
                                        self.display_ty(&expected_ty),
                                        self.display_ty(&fty)
                                    ));
                                }
                            } else {
                                self.reporter.report(
                                    Diagnostic::error(format!(
                                        "Unknown field '{}' in instantiation",
                                        fname
                                    ))
                                    .with_span(*span)
                                    .with_code(ErrorCode::UnknownField),
                                );
                                return Err(format!("Unknown field '{}' in instantiation", fname));
                            }
                        } else {
                            self.reporter.report(
                                Diagnostic::error("Struct instantiation requires named arguments")
                                    .with_span(*span)
                                    .with_code(ErrorCode::InvalidArguments),
                            );
                            return Err("Struct instantiation requires named arguments".to_string());
                        }
                    }
                    def_map.retain(|_, ty| !matches!(ty, Ty::Optional(_)));
                    if !def_map.is_empty() {
                        self.reporter.report(
                            Diagnostic::error(format!(
                                "Missing fields in struct instantiation: {:?}",
                                def_map.keys()
                            ))
                            .with_span(*span)
                            .with_code(ErrorCode::MissingFields),
                        );
                        return Err(format!(
                            "Missing fields in struct instantiation: {:?}",
                            def_map.keys()
                        ));
                    }
                    return Ok(Ty::Struct(id));
                }

                if let Ty::Class(id) | Ty::Actor(id) = callee_ty {
                    let is_actor = matches!(callee_ty, Ty::Actor(_));
                    let mut class_name = "";
                    for (name, &(nid, _)) in &self.named_types {
                        if nid == id {
                            class_name = name;
                            break;
                        }
                    }
                    let init_name = format!("{}_init", class_name);

                    if let Some(Ty::Function(param_tys, _)) =
                        self.methods_env.get(&init_name).cloned()
                    {
                        if args.len() != param_tys.len() - 1 {
                            self.reporter.report(
                                Diagnostic::error(format!(
                                    "{} takes {} arguments, got {}",
                                    init_name,
                                    param_tys.len() - 1,
                                    args.len()
                                ))
                                .with_span(*span)
                                .with_code(ErrorCode::ArityMismatch),
                            );
                            return Err(format!(
                                "{} takes {} arguments, got {}",
                                init_name,
                                param_tys.len() - 1,
                                args.len()
                            ));
                        }
                        for (i, pace_hir::HirCallArg { expr: fexpr, .. }) in args.iter().enumerate()
                        {
                            let fty = self.check_expr(fexpr)?;
                            if fty != param_tys[i + 1] {
                                self.reporter.report(
                                    Diagnostic::error(format!(
                                        "Argument {} expects type {}, got {}",
                                        i,
                                        self.display_ty(&param_tys[i + 1]),
                                        self.display_ty(&fty)
                                    ))
                                    .with_span(*span)
                                    .with_code(ErrorCode::TypeMismatch),
                                );
                                return Err(format!(
                                    "Argument {} expects type {}, got {}",
                                    i,
                                    self.display_ty(&param_tys[i + 1]),
                                    self.display_ty(&fty)
                                ));
                            }
                        }
                        if is_actor { return Ok(Ty::Actor(id)); } else { return Ok(Ty::Class(id)); }
                    }

                    let def_fields = match self.class_defs.get(&id) {
                        Some(fields) => fields.clone(),
                        None => {
                            let type_name = if class_name.is_empty() {
                                format!("{:?}", id)
                            } else {
                                class_name.to_string()
                            };
                            self.reporter.report(
                                Diagnostic::error(format!("Undefined class: '{}'", type_name))
                                    .with_span(*span)
                                    .with_code(ErrorCode::UndefinedType),
                            );
                            return Err(format!("Undefined class: '{}'", type_name));
                        }
                    };
                    let mut def_map: std::collections::HashMap<_, _> = def_fields
                        .into_iter()
                        .map(|f| (f.name.clone(), f.ty.clone()))
                        .collect();
                    for pace_hir::HirCallArg { label, expr: fexpr } in args {
                        let prev_expected = self.current_expected_ty.take();
                        self.current_expected_ty = label
                            .as_ref()
                            .and_then(|fname| def_map.get(fname.as_str()).cloned());
                        let fty = self.check_expr(fexpr)?;
                        self.current_expected_ty = prev_expected;
                        if let Some(fname) = label {
                            if let Some(expected_ty) = def_map.remove(fname.as_str()) {
                                if !self.is_assignable(&expected_ty, &fty) {
                                    self.reporter.report(
                                        Diagnostic::error(format!(
                                            "Field '{}' expects type {}, got {}",
                                            fname,
                                            self.display_ty(&expected_ty),
                                            self.display_ty(&fty)
                                        ))
                                        .with_span(*span)
                                        .with_code(ErrorCode::TypeMismatch),
                                    );
                                    return Err(format!(
                                        "Field '{}' expects type {}, got {}",
                                        fname,
                                        self.display_ty(&expected_ty),
                                        self.display_ty(&fty)
                                    ));
                                }
                            } else {
                                self.reporter.report(
                                    Diagnostic::error(format!(
                                        "Unknown field '{}' in instantiation",
                                        fname
                                    ))
                                    .with_span(*span)
                                    .with_code(ErrorCode::UnknownField),
                                );
                                return Err(format!("Unknown field '{}' in instantiation", fname));
                            }
                        } else {
                            self.reporter.report(
                                Diagnostic::error("Class instantiation requires named arguments")
                                    .with_span(*span)
                                    .with_code(ErrorCode::InvalidArguments),
                            );
                            return Err("Class instantiation requires named arguments".to_string());
                        }
                    }
                    def_map.retain(|_, ty| !matches!(ty, Ty::Optional(_)));
                    if !def_map.is_empty() {
                        self.reporter.report(
                            Diagnostic::error(format!(
                                "Missing fields in class instantiation: {:?}",
                                def_map.keys()
                            ))
                            .with_span(*span)
                            .with_code(ErrorCode::MissingFields),
                        );
                        return Err(format!(
                            "Missing fields in class instantiation: {:?}",
                            def_map.keys()
                        ));
                    }
                    if is_actor { return Ok(Ty::Actor(id)); } else { return Ok(Ty::Class(id)); }
                }

                if let Ty::Function(_, ret_ty) = &callee_ty
                    && let Ty::Enum(enum_id) = **ret_ty
                {
                    // Enum variant instantiation!
                    let variants = match self.enum_defs.get(&enum_id) {
                        Some(v) => v,
                        None => {
                            let mut enum_name = String::new();
                            for (name, &(nid, _)) in &self.named_types {
                                if nid == enum_id {
                                    enum_name = name.clone();
                                    break;
                                }
                            }
                            let type_name = if enum_name.is_empty() {
                                format!("{:?}", enum_id)
                            } else {
                                enum_name
                            };
                            self.reporter.report(
                                Diagnostic::error(format!("Undefined enum: '{}'", type_name))
                                    .with_span(*span)
                                    .with_code(ErrorCode::UndefinedType),
                            );
                            return Err(format!("Undefined enum: '{}'", type_name));
                        }
                    };
                    let mut variant_name = "";
                    if let Expr::MemberAccess { member, .. } = &**callee {
                        variant_name = member;
                    }
                    let mut def_fields = Vec::new();
                    for v in variants {
                        if v.name == variant_name
                            && let Some(f) = &v.fields
                        {
                            def_fields = f.clone();
                        }
                    }
                    let mut def_map = std::collections::HashMap::new();
                    for (n, t) in def_fields {
                        def_map.insert(n, self.resolve_type(&t)?);
                    }
                    for pace_hir::HirCallArg { label, expr: fexpr } in args {
                        let prev_expected = self.current_expected_ty.take();
                        self.current_expected_ty = label
                            .as_ref()
                            .and_then(|fname| def_map.get(fname).cloned());
                        let fty = self.check_expr(fexpr)?;
                        self.current_expected_ty = prev_expected;
                        if let Some(fname) = label {
                            if let Some(expected_ty) = def_map.remove(fname) {
                                if fty != expected_ty {
                                    self.reporter.report(
                                        Diagnostic::error(format!(
                                            "Field '{}' expects type {}, got {}",
                                            fname,
                                            self.display_ty(&expected_ty),
                                            self.display_ty(&fty)
                                        ))
                                        .with_span(*span)
                                        .with_code(ErrorCode::TypeMismatch),
                                    );
                                    return Err(format!(
                                        "Field '{}' expects type {}, got {}",
                                        fname,
                                        self.display_ty(&expected_ty),
                                        self.display_ty(&fty)
                                    ));
                                }
                            } else {
                                self.reporter.report(
                                    Diagnostic::error(format!(
                                        "Unknown field '{}' in variant instantiation",
                                        fname
                                    ))
                                    .with_span(*span)
                                    .with_code(ErrorCode::UnknownField),
                                );
                                return Err(format!(
                                    "Unknown field '{}' in variant instantiation",
                                    fname
                                ));
                            }
                        } else {
                            self.reporter.report(
                                Diagnostic::error("Variant instantiation requires named arguments")
                                    .with_span(*span)
                                    .with_code(ErrorCode::InvalidArguments),
                            );
                            return Err(
                                "Variant instantiation requires named arguments".to_string()
                            );
                        }
                    }
                    def_map.retain(|_, ty| !matches!(ty, Ty::Optional(_)));
                    if !def_map.is_empty() {
                        self.reporter.report(
                            Diagnostic::error(format!(
                                "Missing fields in variant instantiation: {:?}",
                                def_map.keys()
                            ))
                            .with_span(*span)
                            .with_code(ErrorCode::MissingFields),
                        );
                        return Err(format!(
                            "Missing fields in variant instantiation: {:?}",
                            def_map.keys()
                        ));
                    }
                    return Ok(Ty::Enum(enum_id));
                }

                for pace_hir::HirCallArg { expr: arg, .. } in args {
                    self.check_expr(arg)?;
                }
                self.function_calls.push((*span, callee_ty.clone()));
                if let Ty::Function(_, ret_ty) | Ty::Closure(_, ret_ty) = callee_ty {
                    return Ok(*ret_ty);
                }
                if let Ty::List(_) | Ty::Map(_, _) | Ty::Set(_) | Ty::StringBuilder = callee_ty {
                    return Ok(callee_ty);
                }
                Ok(Ty::Int) // Fallback for MVP
            }
            Expr::BuiltinCall(name, args, _) => {
                let mut arg_types = Vec::new();
                for arg in args {
                    arg_types.push(self.check_expr(arg)?);
                }

                if name == "print" || name == "println" {
                    if arg_types.len() != 1 {
                        return Err(format!("{} takes exactly 1 argument", name));
                    }
                    Ok(Ty::Int)
                } else if name.as_str().starts_with("pace_") {
                    match name.as_str() {
                        "pace_eprintln" | "pace_env_set" | "pace_time_sleep_ms" => Ok(Ty::Void),
                        "pace_process_exit" => Ok(Ty::Never),
                        "pace_file_exists" | "pace_dir_exists" | "pace_env_has" => Ok(Ty::Bool),
                        "pace_time_now_ms" => Ok(Ty::Int),
                        "pace_os_name" | "pace_process_cwd" => Ok(Ty::String),
                        "pace_env_get" => Ok(Ty::Optional(Box::new(Ty::String))),
                        "pace_process_args" => Ok(Ty::List(Box::new(Ty::String))),
                        "pace_read_line" | "pace_file_read_text" => {
                            Ok(Ty::Result(Box::new(Ty::String), Box::new(Ty::String)))
                        }
                        "pace_file_write_text" | "pace_dir_make" => {
                            Ok(Ty::Result(Box::new(Ty::Void), Box::new(Ty::String)))
                        }
                        "pace_math_sqrt" | "pace_math_pow" | "pace_math_abs"
                        | "pace_math_round" | "pace_math_floor" | "pace_math_ceil"
                        | "pace_math_sin" | "pace_math_cos" | "pace_math_tan" => Ok(Ty::Float),
                        _ => Err(format!("Unknown runtime builtin '{}'", name)),
                    }
                } else if name == "assert" {
                    if arg_types.is_empty() || arg_types.len() > 2 {
                        return Err("assert takes 1 or 2 arguments: assert(condition, [message])".to_string());
                    }
                    if !matches!(arg_types[0], Ty::Bool | Ty::Int) {
                        return Err("assert condition must evaluate to a boolean".to_string());
                    }
                    if arg_types.len() == 2 && !matches!(arg_types[1], Ty::String) {
                        return Err("assert message must be a string".to_string());
                    }
                    Ok(Ty::Void)
                } else if name == "sleep" {
                    if arg_types.len() != 1 {
                        return Err(format!("{} takes exactly 1 argument", name));
                    }
                    if !matches!(arg_types[0], Ty::Int) {
                        return Err(format!("{} takes an int", name));
                    }
                    Ok(Ty::Future(Box::new(Ty::Void)))
                } else if name == "ok" {
                    if arg_types.len() > 1 {
                        return Err("ok() takes 0 or 1 arguments".to_string());
                    }
                    let expected = self
                        .current_expected_ty
                        .as_ref()
                        .or(self.current_return_ty.as_ref())
                        .cloned();
                    if let Some(Ty::Result(ok_ty, err_ty)) = expected {
                        let actual_ok = arg_types.first().cloned().unwrap_or(Ty::Void);
                        if actual_ok != *ok_ty {
                            return Err(format!(
                                "Type mismatch in ok(): expected {}, found {}",
                                self.display_ty(&ok_ty),
                                self.display_ty(&actual_ok)
                            ));
                        }
                        Ok(Ty::Result(ok_ty, err_ty))
                    } else {
                        let val_ty = arg_types.first().cloned().unwrap_or(Ty::Void);
                        Ok(Ty::Result(Box::new(val_ty), Box::new(Ty::Void)))
                    }
                } else if name == "error" {
                    if arg_types.len() != 1 {
                        return Err("error() takes exactly 1 argument".to_string());
                    }
                    let expected = self
                        .current_expected_ty
                        .as_ref()
                        .or(self.current_return_ty.as_ref())
                        .cloned();
                    if let Some(Ty::Result(ok_ty, err_ty)) = expected {
                        if arg_types[0] != *err_ty {
                            return Err(format!(
                                "Type mismatch in error(): expected {}, found {}",
                                self.display_ty(&err_ty),
                                self.display_ty(&arg_types[0])
                            ));
                        }
                        Ok(Ty::Result(ok_ty, err_ty))
                    } else {
                        Ok(Ty::Result(Box::new(Ty::Void), Box::new(arg_types[0].clone())))
                    }
                } else {
                    Err(format!("Unknown builtin: {}", name))
                }
            }
            Expr::If {
                cond,
                then_block,
                else_block,
                ..
            } => {
                self.check_expr(cond)?;
                self.check_block(then_block, None)?;
                if let Some(else_b) = else_block {
                    self.check_block(else_b, None)?;
                }
                Ok(Ty::Int)
            }
            Expr::While { cond, body, .. } => {
                self.check_expr(cond)?;
                self.loop_depth += 1;
                let res = self.check_block(body, None);
                self.loop_depth -= 1;
                res?;
                Ok(Ty::Int)
            }
            Expr::Assign {
                target,
                value,
                span,
            } => {
                // Determine target type without checking initialization for Idents
                let target_ty = match &**target {
                    Expr::Ident(id, _, _, _) => {
                        self.used_bindings.insert(*id);
                        self.env
                            .get(id)
                            .cloned()
                            .ok_or("Cannot infer type for unbound variable".to_string())?
                    }
                    _ => self.check_expr(target)?,
                };

                // Mutability check
                if let Expr::Ident(id, _, _, _) = &**target {
                    let was_initialized = self.initialized_bindings.contains(id);
                    self.initialized_bindings.insert(*id); // Mark as initialized upon assignment
                    if let Some(&is_mut) = self.mutability_env.get(id)
                        && !is_mut
                        && was_initialized
                    {
                        self.reporter.report(Diagnostic::error("Cannot reassign immutable variable")
                                .with_span(*span)
                                .with_code(ErrorCode::ImmutableAssignment)
                                .with_hint("Declare this variable with 'var' instead of 'let' to make it mutable"));
                    }
                } else if let Expr::MemberAccess { object, member, .. } = &**target {
                    if let Expr::Ident(id, _, _, _) = &**object
                        && let Some(&is_mut) = self.mutability_env.get(id)
                        && !is_mut
                    {
                        self.reporter.report(Diagnostic::error("Cannot mutate field of immutable variable")
                                    .with_span(*span)
                                    .with_code(ErrorCode::ImmutableAssignment)
                                    .with_hint("Declare this variable with 'var' instead of 'let' to make it mutable"));
                    }

                    let obj_ty = self.check_expr(object)?;
                    let mut field_is_mut = true;
                    if let Ty::Struct(id) | Ty::Class(id) = obj_ty {
                        let fields = if matches!(obj_ty, Ty::Class(_)) {
                            self.class_defs.get(&id)
                        } else {
                            self.struct_defs.get(&id)
                        };
                        if let Some(fields) = fields {
                            for ResolvedField {
                                name: fname,
                                is_mut,
                                ..
                            } in fields
                            {
                                if fname == member {
                                    field_is_mut = *is_mut;
                                    break;
                                }
                            }
                        }
                    }

                    if !field_is_mut {
                        let mut allowed = false;
                        if let Expr::Ident(_, name, _, _) = &**object
                            && name == "self"
                            && let Some(fn_name) = &self.current_fn_name
                            && (fn_name.ends_with("_init") || fn_name == "init")
                        {
                            allowed = true;
                        }

                        if !allowed {
                            self.reporter.report(Diagnostic::error(format!("Cannot reassign immutable field '{}'", member))
                                .with_span(*span)
                                .with_code(ErrorCode::ImmutableAssignment)
                                .with_hint("Declare this field with 'var' instead of 'let' to make it mutable"));
                        }
                    }
                }

                let prev_expected = self.current_expected_ty.take();
                self.current_expected_ty = Some(target_ty.clone());
                let val_ty_res = self.check_expr(value);
                self.current_expected_ty = prev_expected;

                let val_ty = val_ty_res?;
                let mut type_matches = target_ty == val_ty;
                if !type_matches {
                    if target_ty == Ty::Optional(Box::new(val_ty.clone())) {
                        type_matches = true;
                    } else if let (Ty::Optional(_), Ty::Optional(inner_val)) = (&target_ty, &val_ty)
                        && **inner_val == Ty::Void
                    {
                        type_matches = true;
                    }
                }

                if !type_matches {
                    self.reporter.report(
                        Diagnostic::error(format!(
                            "Type mismatch in assignment: expected {}, got {}",
                            self.display_ty(&target_ty),
                            self.display_ty(&val_ty)
                        ))
                        .with_span(*span),
                    );
                    return Ok(target_ty); // Return target type to continue checking gracefully
                }
                Ok(target_ty)
            }
            Expr::Match {
                subject,
                arms,
                span,
            } => {
                let subject_ty = self.check_expr(subject)?;
                let mut ret_ty = None;
                let mut has_catch_all = false;
                let mut covered_variants = HashSet::new();
                let enum_variants = if let Ty::Enum(enum_id) = subject_ty {
                    self.enum_defs.get(&enum_id).cloned()
                } else {
                    None
                };

                for arm in arms {
                    match &arm.pattern {
                        pace_hir::Pattern::CatchAll(_) => {
                            has_catch_all = true;
                        }
                        pace_hir::Pattern::Variant { name, .. } => {
                            covered_variants.insert(*name);
                        }
                        pace_hir::Pattern::Ident(_, name, _) => {
                            if let Some(variants) = &enum_variants
                                && variants.iter().any(|v| v.name == *name)
                            {
                                covered_variants.insert(*name);
                            } else {
                                has_catch_all = true;
                            }
                        }
                    }

                    let mut arm_env = HashMap::new();
                    if let pace_hir::Pattern::Variant { name, fields, .. } = &arm.pattern
                        && let Some(variants) = &enum_variants
                        && let Some(v) = variants.iter().find(|v| v.name == *name)
                        && let Some(vfields) = &v.fields
                        && let Some(pfields) = fields
                        && vfields.len() == pfields.len()
                    {
                        for (i, (pf_id, _, _)) in pfields.iter().enumerate() {
                            let fty = self.resolve_type(&vfields[i].1)?;
                            arm_env.insert(*pf_id, fty.clone());
                            self.local_types.insert(*pf_id, fty);
                        }
                    }

                    let outer_env = self.env.clone();
                    for (k, v) in arm_env {
                        self.env.insert(k, v);
                    }

                    let arm_ty = self.check_expr(&arm.body)?;
                    self.env = outer_env;

                    if let Some(expected) = &ret_ty {
                        if *expected != arm_ty {
                            if *expected == Ty::Never {
                                ret_ty = Some(arm_ty);
                            } else if arm_ty == Ty::Never {
                                // Keep expected type
                            } else if matches!(expected, Ty::Optional(inner) if **inner == Ty::Void) {
                                ret_ty = Some(match arm_ty {
                                    Ty::Optional(_) => arm_ty,
                                    other => Ty::Optional(Box::new(other)),
                                });
                            } else if matches!(&arm_ty, Ty::Optional(inner) if **inner == Ty::Void) {
                                if !matches!(expected, Ty::Optional(_)) {
                                    ret_ty = Some(Ty::Optional(Box::new(expected.clone())));
                                }
                            } else if *expected == Ty::Optional(Box::new(arm_ty.clone())) {
                                // arm_ty is T, expected is T?; keep T?
                            } else if arm_ty == Ty::Optional(Box::new(expected.clone())) {
                                ret_ty = Some(arm_ty);
                            } else {
                                self.reporter.report(
                                    Diagnostic::error(format!(
                                        "Match arms have incompatible types: {} and {}",
                                        self.display_ty(expected),
                                        self.display_ty(&arm_ty)
                                    ))
                                    .with_span(arm.span)
                                    .with_code(ErrorCode::TypeMismatch),
                                );
                            }
                        }
                    } else {
                        ret_ty = Some(arm_ty);
                    }
                }

                if let Some(variants) = &enum_variants
                    && !has_catch_all
                {
                    let missing: Vec<String> = variants
                        .iter()
                        .filter(|v| !covered_variants.contains(&v.name))
                        .map(|v| v.name.to_string())
                        .collect();
                    if !missing.is_empty() {
                        let msg = format!(
                            "Non-exhaustive match: missing variant '{}'",
                            missing.join("', '")
                        );
                        self.reporter.report(
                            Diagnostic::error(&msg)
                                .with_span(*span)
                                .with_hint("Handle all enum variants or add a '_ =>' catch-all arm"),
                        );
                        return Err(msg);
                    }
                }

                Ok(ret_ty.unwrap_or(Ty::Void))
            }
            Expr::Closure {
                params,
                return_type,
                body,
                captured_vars: _,
                span,
            } => {
                let expected_closure_ty = self.current_expected_ty.clone();
                let mut inferred_param_tys = None;
                if let Some(Ty::Closure(p_tys, _)) = &expected_closure_ty {
                    if p_tys.len() == params.len() {
                        inferred_param_tys = Some(p_tys.clone());
                    }
                }

                let mut param_tys = Vec::new();
                let outer_env = self.env.clone();
                for (i, param) in params.iter().enumerate() {
                    let ty = if let Some(t) = &param.ty {
                        self.resolve_type(t)?
                    } else if let Some(inferred) = &inferred_param_tys {
                        inferred[i].clone()
                    } else {
                        self.reporter.report(
                            pace_errors::Diagnostic::error(format!(
                                "Type of parameter `{}` cannot be inferred, please provide a type annotation",
                                param.name
                            ))
                            .with_span(param.span),
                        );
                        Ty::Int
                    };
                    param_tys.push(ty.clone());
                    self.env.insert(param.id, ty.clone());
                    self.local_types.insert(param.id, ty);
                }

                let ret_ty = match body {
                    pace_hir::ClosureBody::Expr(e) => {
                        let prev_expected = self.current_expected_ty.take();
                        if let Some(Ty::Closure(_, r_ty)) = &expected_closure_ty {
                            self.current_expected_ty = Some((**r_ty).clone());
                        }
                        let t = self.check_expr(e)?;
                        self.current_expected_ty = prev_expected;
                        t
                    }
                    pace_hir::ClosureBody::Block(b) => {
                        let prev_ret = self.current_return_ty.take();
                        let mut block_ret = Ty::Void;
                        if let Some(Ty::Closure(_, r_ty)) = &expected_closure_ty {
                            self.current_return_ty = Some((**r_ty).clone());
                        } else if let Some(t) = return_type {
                            if let Ok(r) = self.resolve_type(t) {
                                self.current_return_ty = Some(r);
                            }
                        }
                        self.check_block(b, None)?;
                        
                        if let Some(inferred_ret) = self.current_return_ty.take() {
                            block_ret = inferred_ret;
                        }
                        self.current_return_ty = prev_ret;
                        
                        block_ret
                    }
                };

                let expected_ret = if let Some(t) = return_type {
                    self.resolve_type(t).unwrap_or(Ty::Void)
                } else {
                    ret_ty.clone()
                };

                if ret_ty != expected_ret && expected_ret != Ty::Void {
                    self.reporter.report(
                        pace_errors::Diagnostic::error(format!(
                            "Closure body returns {}, but is expected to return {}",
                            self.display_ty(&ret_ty),
                            self.display_ty(&expected_ret)
                        ))
                        .with_span(*span),
                    );
                }

                self.env = outer_env;

                Ok(Ty::Closure(param_tys, Box::new(expected_ret)))
            }
            Expr::Await(inner, span) => {
                if !self.in_async_context {
                    self.reporter.report(Diagnostic::error("`await` can only be used inside `async` functions or closures")
                        .with_span(*span));
                }
                let inner_ty = self.check_expr(inner)?;
                match inner_ty {
                    Ty::Future(t) => Ok(*t),
                    _ => {
                        self.reporter.report(Diagnostic::error(format!("`await` requires a Future type, found {}", self.display_ty(&inner_ty)))
                            .with_span(*span));
                        Ok(Ty::Void)
                    }
                }
            }
            Expr::Try(inner, span) => {
                let inner_ty = self.check_expr(inner)?;
                match inner_ty {
                    Ty::Result(ok_ty, err_ty) => {
                        if let Some(ref ret_ty) = self.current_return_ty {
                            if let Ty::Result(_, expected_err) = ret_ty {
                                if *err_ty != **expected_err {
                                    self.reporter.report(
                                        Diagnostic::error(format!(
                                            "Error type mismatch in '?' operator: cannot propagate error type '{}' from function returning error type '{}'",
                                            self.display_ty(&err_ty),
                                            self.display_ty(expected_err),
                                        ))
                                        .with_span(*span),
                                    );
                                    return Err("Error type mismatch in '?'".to_string());
                                }
                            } else {
                                self.reporter.report(
                                    Diagnostic::error(format!(
                                        "The '?' operator can only be used in a function that returns 'Result', but enclosing function returns '{}'",
                                        self.display_ty(ret_ty),
                                    ))
                                    .with_span(*span),
                                );
                                return Err("Cannot use '?' in non-Result function".to_string());
                            }
                        } else {
                            self.reporter.report(
                                Diagnostic::error(
                                    "The '?' operator can only be used inside a function",
                                )
                                .with_span(*span),
                            );
                            return Err("Cannot use '?' outside of a function".to_string());
                        }
                        Ok(*ok_ty)
                    }
                    Ty::Optional(inner_val) => {
                        if let Some(ref ret_ty) = self.current_return_ty {
                            if !matches!(ret_ty, Ty::Optional(_)) {
                                self.reporter.report(
                                    Diagnostic::error(format!(
                                        "The '?' operator on an optional type can only be used in a function that returns an optional, but enclosing function returns '{}'",
                                        self.display_ty(ret_ty),
                                    ))
                                    .with_span(*span),
                                );
                                return Err("Cannot use '?' on optional in non-optional function".to_string());
                            }
                        } else {
                            self.reporter.report(
                                Diagnostic::error(
                                    "The '?' operator can only be used inside a function",
                                )
                                .with_span(*span),
                            );
                            return Err("Cannot use '?' outside of a function".to_string());
                        }
                        Ok(*inner_val)
                    }
                    other => {
                        self.reporter.report(
                            Diagnostic::error(format!(
                                "Cannot use '?' operator on type '{}'; expected 'Result' or optional type",
                                self.display_ty(&other),
                            ))
                            .with_span(*span),
                        );
                        Err("Invalid type for '?' operator".to_string())
                    }
                }
            }
            Expr::ListLiteral(elements, _span) => {
                if elements.is_empty() {
                    if let Some(Ty::List(inner)) = &self.current_expected_ty {
                        return Ok(Ty::List(inner.clone()));
                    }
                    return Ok(Ty::List(Box::new(Ty::Int)));
                }

                let mut first_elem_ty = self.check_expr(&elements[0])?;
                for elem in &elements[1..] {
                    let prev_exp = self.current_expected_ty.take();
                    self.current_expected_ty = Some(first_elem_ty.clone());
                    let elem_ty = self.check_expr(elem)?;
                    self.current_expected_ty = prev_exp;
                    if let Some(unified) = self.unify_types(&first_elem_ty, &elem_ty) {
                        first_elem_ty = unified;
                    } else {
                        self.reporter.report(
                            Diagnostic::error(format!(
                                "List elements must have consistent types: expected {}, found {}",
                                self.display_ty(&first_elem_ty),
                                self.display_ty(&elem_ty)
                            ))
                            .with_span(elem.span()),
                        );
                    }
                }
                Ok(Ty::List(Box::new(first_elem_ty)))
            }
            Expr::MapLiteral(entries, _span) => {
                if entries.is_empty() {
                    if let Some(Ty::Map(k, v)) = &self.current_expected_ty {
                        return Ok(Ty::Map(k.clone(), v.clone()));
                    }
                    return Ok(Ty::Map(Box::new(Ty::String), Box::new(Ty::Int)));
                }

                let first_k_ty = self.check_expr(&entries[0].0)?;
                let mut first_v_ty = self.check_expr(&entries[0].1)?;

                for (k, v) in &entries[1..] {
                    let k_ty = self.check_expr(k)?;
                    let v_ty = self.check_expr(v)?;
                    if k_ty != first_k_ty {
                        self.reporter.report(
                            Diagnostic::error(format!(
                                "Map keys must have consistent types: expected {}, found {}",
                                self.display_ty(&first_k_ty),
                                self.display_ty(&k_ty)
                            ))
                            .with_span(k.span()),
                        );
                    }
                    if let Some(unified) = self.unify_types(&first_v_ty, &v_ty) {
                        first_v_ty = unified;
                    } else {
                        self.reporter.report(
                            Diagnostic::error(format!(
                                "Map values must have consistent types: expected {}, found {}",
                                self.display_ty(&first_v_ty),
                                self.display_ty(&v_ty)
                            ))
                            .with_span(v.span()),
                        );
                    }
                }
                Ok(Ty::Map(Box::new(first_k_ty), Box::new(first_v_ty)))
            }
            Expr::Index { target, index, span } => {
                let target_ty = self.check_expr(target)?;
                let prev_exp = self.current_expected_ty.take();
                let index_ty = match &target_ty {
                    Ty::List(_) => {
                        self.current_expected_ty = Some(Ty::Int);
                        let ty = self.check_expr(index)?;
                        self.current_expected_ty = prev_exp;
                        ty
                    }
                    Ty::Map(k, _) => {
                        self.current_expected_ty = Some((**k).clone());
                        let ty = self.check_expr(index)?;
                        self.current_expected_ty = prev_exp;
                        ty
                    }
                    _ => {
                        self.current_expected_ty = prev_exp;
                        self.check_expr(index)?
                    }
                };

                match target_ty {
                    Ty::List(elem) => {
                        if index_ty != Ty::Int {
                            self.reporter.report(
                                Diagnostic::error(format!(
                                    "List index must be int, got {}",
                                    self.display_ty(&index_ty)
                                ))
                                .with_span(index.span()),
                            );
                        }
                        Ok(*elem)
                    }
                    Ty::Map(k, v) => {
                        if index_ty != *k {
                            self.reporter.report(
                                Diagnostic::error(format!(
                                    "Map key must be {}, got {}",
                                    self.display_ty(&k),
                                    self.display_ty(&index_ty)
                                ))
                                .with_span(index.span()),
                            );
                        }
                        Ok(Ty::Optional(v))
                    }
                    other => {
                        self.reporter.report(
                            Diagnostic::error(format!(
                                "Cannot index into type '{}'",
                                self.display_ty(&other)
                            ))
                            .with_span(*span),
                        );
                        Err(format!("Cannot index into type '{}'", self.display_ty(&other)))
                    }
                }
            }
        }
    }

}
