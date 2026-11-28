use super::{ModuleScope, ResolvedField, ResolvedMethod, VTableEntry};
use pace_hir::{Decl, HirId, Program};
use std::collections::HashMap;

use crate::ty::Ty;
use pace_errors::{Diagnostic, ErrorCode};

use super::TypeChecker;

impl TypeChecker {
    pub fn check_program(&mut self, program: &Program) -> Result<(), String> {
        // 0. Initialize module scopes
        for module_name in &program.module_order {
            let module = match program.modules.get(module_name) {
                Some(m) => m,
                None => return Err(format!("ICE: Module {} not found", module_name)),
            };
            let mut scope = ModuleScope::default();
            for decl in &module.declarations {
                if let Decl::Import { path, alias, .. } = decl
                    && let Some(target_module) = path.first()
                {
                    if let Some(a) = alias {
                        scope
                            .aliases
                            .insert(a.to_string(), target_module.to_string());
                    } else {
                        scope.wildcard_imports.push(target_module.to_string());
                    }
                }
            }
            self.module_scopes.insert(module_name.to_string(), scope);
        }

        let mut declarations: Vec<(String, Decl)> = Vec::new();
        for module_name in &program.module_order {
            let module = match program.modules.get(module_name) {
                Some(m) => m,
                None => return Err(format!("ICE: Module {} not found", module_name)),
            };
            for decl in &module.declarations {
                declarations.push((module_name.to_string(), decl.clone()));
            }
        }

        let mut has_main = false;

        for (_, decl) in &declarations {
            if let Decl::Function { name, .. } = decl {
                if name == "main" {
                    has_main = true;
                }
            } else if let Decl::Expr(_, span) = decl {
                self.reporter.report(
                    Diagnostic::error("Top-level executable statements are not allowed")
                        .with_span(*span)
                        .with_hint("Wrap this code in a 'fn main()' block"),
                );
            }
        }

        // MVP: Assume it's an executable if it's not a library.
        // For now we will warn if main is missing instead of error, so we don't break old tests without main yet.
        if !has_main && !self.is_lib {
            self.reporter.report(
                Diagnostic::warning("No 'main' function found")
                    .with_hint("Executables must have an entry point 'fn main()'"),
            );
        }

        // 1. Gather all top-level types (Structs/Classes/Actors/Enums/Traits)
        for (module_name, decl) in &declarations {
            self.current_module = Some(module_name.to_string());
            match decl {
                Decl::Struct {
                    id,
                    name,
                    is_private,
                    generic_params,
                    span,
                    ..
                } => {
                    let mangled_name = format!("{}_{}", module_name, name);
                    self.display_names.insert(mangled_name.clone(), name.to_string());
                    if *is_private {
                        if let Some(scope) = self.module_scopes.get_mut(module_name) {
                            scope.private_symbols.insert(name.to_string());
                        }
                    }
                    if generic_params.is_some() {
                        self.generic_templates
                            .insert(mangled_name.clone(), decl.clone());
                        self.generic_templates_by_id
                            .insert(*id, mangled_name.clone());
                        if let Some(scope) = self.module_scopes.get_mut(module_name) {
                            scope.visible_symbols.insert(name.to_string(), mangled_name);
                        }
                        continue;
                    }
                    self.named_types.insert(mangled_name.clone(), (*id, 0));
                    self.global_spans.insert(mangled_name.clone(), *span);
                    if let Some(scope) = self.module_scopes.get_mut(module_name) {
                        scope
                            .visible_symbols
                            .insert(name.to_string(), mangled_name);
                    }
                }
                Decl::Class {
                    id,
                    name,
                    is_private,
                    generic_params,
                    span,
                    ..
                } => {
                    let mangled_name = format!("{}_{}", module_name, name);
                    self.display_names.insert(mangled_name.clone(), name.to_string());
                    if *is_private {
                        if let Some(scope) = self.module_scopes.get_mut(module_name) {
                            scope.private_symbols.insert(name.to_string());
                        }
                    }
                    if generic_params.is_some() {
                        self.generic_templates
                            .insert(mangled_name.clone(), decl.clone());
                        self.generic_templates_by_id
                            .insert(*id, mangled_name.clone());
                        if let Some(scope) = self.module_scopes.get_mut(module_name) {
                            scope.visible_symbols.insert(name.to_string(), mangled_name);
                        }
                        continue;
                    }
                    self.named_types.insert(mangled_name.clone(), (*id, 1));
                    self.global_spans.insert(mangled_name.clone(), *span);
                    if let Some(scope) = self.module_scopes.get_mut(module_name) {
                        scope
                            .visible_symbols
                            .insert(name.to_string(), mangled_name);
                    }
                }
                Decl::Actor {
                    id,
                    name,
                    is_private,
                    generic_params,
                    span,
                    ..
                } => {
                    let mangled_name = format!("{}_{}", module_name, name);
                    self.display_names.insert(mangled_name.clone(), name.to_string());
                    if *is_private {
                        if let Some(scope) = self.module_scopes.get_mut(module_name) {
                            scope.private_symbols.insert(name.to_string());
                        }
                    }
                    if generic_params.is_some() {
                        self.generic_templates
                            .insert(mangled_name.clone(), decl.clone());
                        self.generic_templates_by_id
                            .insert(*id, mangled_name.clone());
                        if let Some(scope) = self.module_scopes.get_mut(module_name) {
                            scope.visible_symbols.insert(name.to_string(), mangled_name);
                        }
                        continue;
                    }
                    self.named_types.insert(mangled_name.clone(), (*id, 3));
                    self.global_spans.insert(mangled_name.clone(), *span);
                    if let Some(scope) = self.module_scopes.get_mut(module_name) {
                        scope
                            .visible_symbols
                            .insert(name.to_string(), mangled_name);
                    }
                }
                Decl::Trait { name, .. } => {
                    self.trait_defs.insert(name.to_string(), decl.clone());
                }
                Decl::Enum {
                    id,
                    name,
                    is_private,
                    generic_params,
                    span,
                    ..
                } => {
                    let mangled_name = format!("{}_{}", module_name, name);
                    self.display_names.insert(mangled_name.clone(), name.to_string());
                    if *is_private {
                        if let Some(scope) = self.module_scopes.get_mut(module_name) {
                            scope.private_symbols.insert(name.to_string());
                        }
                    }
                    if generic_params.is_some() {
                        self.generic_templates
                            .insert(mangled_name.clone(), decl.clone());
                        self.generic_templates_by_id
                            .insert(*id, mangled_name.clone());
                        if let Some(scope) = self.module_scopes.get_mut(module_name) {
                            scope.visible_symbols.insert(name.to_string(), mangled_name);
                        }
                        continue;
                    }
                    self.named_types.insert(mangled_name.clone(), (*id, 2));
                    self.global_spans.insert(mangled_name.clone(), *span);
                    if let Some(scope) = self.module_scopes.get_mut(module_name) {
                        scope.visible_symbols.insert(name.to_string(), mangled_name);
                    }
                }
                _ => {}
            }
        }

        // 1.5. Register static fields, const fields, methods, and enum variants
        for (module_name, decl) in &declarations {
            self.current_module = Some(module_name.to_string());
            match decl {
                Decl::Struct {
                    name,
                    generic_params,
                    static_fields,
                    const_fields,
                    methods,
                    ..
                } => {
                    if generic_params.is_some() {
                        continue;
                    }
                    let mangled_name = format!("{}_{}", module_name, name);
                    for pace_hir::HirStaticFieldDef {
                        name: sf_name,
                        ty: sf_ty,
                        ..
                    } in static_fields
                    {
                        let resolved_ty = self.resolve_type(sf_ty).map_err(|e| {
                            self.reporter.report(pace_errors::Diagnostic::error(e).with_span(sf_ty.span()));
                            "Type resolution failed".to_string()
                        })?;
                        self.static_fields_env
                            .insert(format!("{}_{}", mangled_name, sf_name), resolved_ty);
                    }
                    for pace_hir::HirConstFieldDef {
                        name: cf_name,
                        ty: cf_ty,
                        ..
                    } in const_fields
                    {
                        let resolved_ty = self.resolve_type(cf_ty).map_err(|e| {
                            self.reporter.report(pace_errors::Diagnostic::error(e).with_span(cf_ty.span()));
                            "Type resolution failed".to_string()
                        })?;
                        self.const_env
                            .insert(format!("{}_{}", mangled_name, cf_name), resolved_ty);
                    }
                    for method in methods {
                        if let Decl::Function {
                            name: m_name,
                            params,
                            return_type,
                            id: m_id,
                            span: m_span,
                            ..
                        } = method
                        {
                            let mut param_tys = Vec::new();
                            for (_, _, pty) in params {
                                let p_ty = self.resolve_type(pty).map_err(|e| {
                                    self.reporter.report(pace_errors::Diagnostic::error(e).with_span(pty.span()));
                                    "Type resolution failed".to_string()
                                })?;
                                param_tys.push(p_ty);
                            }
                            let ret_ty = if let Some(r) = return_type {
                                self.resolve_type(r).map_err(|e| {
                                    self.reporter.report(pace_errors::Diagnostic::error(e).with_span(r.span()));
                                    "Type resolution failed".to_string()
                                })?
                            } else {
                                Ty::Void
                            };
                            let short_method_name =
                                m_name.trim_prefix(&format!("{}_", name));
                            let method_mangled_name =
                                format!("{}_{}", mangled_name, short_method_name);
                            self.resolved_global_names.insert(*m_id, method_mangled_name.clone());
                            self.env.insert(
                                *m_id,
                                Ty::Function(param_tys.clone(), Box::new(ret_ty.clone())),
                            );
                            self.methods_env.insert(
                                method_mangled_name.clone(),
                                Ty::Function(param_tys, Box::new(ret_ty)),
                            );
                            self.global_spans.insert(method_mangled_name, *m_span);
                        }
                    }
                }
                Decl::Class {
                    name,
                    generic_params,
                    static_fields,
                    const_fields,
                    methods,
                    ..
                } => {
                    if generic_params.is_some() {
                        continue;
                    }
                    let mangled_name = format!("{}_{}", module_name, name);
                    for pace_hir::HirStaticFieldDef {
                        name: sf_name,
                        ty: sf_ty,
                        ..
                    } in static_fields
                    {
                        let resolved_ty = self.resolve_type(sf_ty).map_err(|e| {
                            self.reporter.report(pace_errors::Diagnostic::error(e).with_span(sf_ty.span()));
                            "Type resolution failed".to_string()
                        })?;
                        self.static_fields_env
                            .insert(format!("{}_{}", mangled_name, sf_name), resolved_ty);
                    }
                    for pace_hir::HirConstFieldDef {
                        name: cf_name,
                        ty: cf_ty,
                        ..
                    } in const_fields
                    {
                        let resolved_ty = self.resolve_type(cf_ty).map_err(|e| {
                            self.reporter.report(pace_errors::Diagnostic::error(e).with_span(cf_ty.span()));
                            "Type resolution failed".to_string()
                        })?;
                        self.const_env
                            .insert(format!("{}_{}", mangled_name, cf_name), resolved_ty);
                    }
                    for method in methods {
                        if let Decl::Function {
                            name: m_name,
                            params,
                            return_type,
                            id: m_id,
                            span: m_span,
                            ..
                        } = method
                        {
                            let mut param_tys = Vec::new();
                            for (_, _, pty) in params {
                                let p_ty = self.resolve_type(pty).map_err(|e| {
                                    self.reporter.report(pace_errors::Diagnostic::error(e).with_span(pty.span()));
                                    "Type resolution failed".to_string()
                                })?;
                                param_tys.push(p_ty);
                            }
                            let ret_ty = if let Some(r) = return_type {
                                self.resolve_type(r).map_err(|e| {
                                    self.reporter.report(pace_errors::Diagnostic::error(e).with_span(r.span()));
                                    "Type resolution failed".to_string()
                                })?
                            } else {
                                Ty::Void
                            };
                            let short_method_name =
                                m_name.trim_prefix(&format!("{}_", name));
                            let method_mangled_name =
                                format!("{}_{}", mangled_name, short_method_name);
                            self.resolved_global_names.insert(*m_id, method_mangled_name.clone());
                            self.env.insert(
                                *m_id,
                                Ty::Function(param_tys.clone(), Box::new(ret_ty.clone())),
                            );
                            self.methods_env.insert(
                                method_mangled_name.clone(),
                                Ty::Function(param_tys, Box::new(ret_ty)),
                            );
                            self.global_spans.insert(method_mangled_name, *m_span);
                        }
                    }
                }
                Decl::Actor {
                    name,
                    generic_params,
                    methods,
                    ..
                } => {
                    if generic_params.is_some() {
                        continue;
                    }
                    let mangled_name = format!("{}_{}", module_name, name);
                    for method in methods {
                        if let Decl::Function {
                            name: m_name,
                            params,
                            return_type,
                            id: m_id,
                            span: m_span,
                            ..
                        } = method
                        {
                            let mut param_tys = Vec::new();
                            for (_, _, pty) in params {
                                let p_ty = self.resolve_type(pty).map_err(|e| {
                                    self.reporter.report(pace_errors::Diagnostic::error(e).with_span(pty.span()));
                                    "Type resolution failed".to_string()
                                })?;
                                param_tys.push(p_ty);
                            }
                            let ret_ty = if let Some(r) = return_type {
                                self.resolve_type(r).map_err(|e| {
                                    self.reporter.report(pace_errors::Diagnostic::error(e).with_span(r.span()));
                                    "Type resolution failed".to_string()
                                })?
                            } else {
                                Ty::Void
                            };
                            let short_method_name =
                                m_name.trim_prefix(&format!("{}_", name));
                            let method_mangled_name =
                                format!("{}_{}", mangled_name, short_method_name);
                            self.resolved_global_names.insert(*m_id, method_mangled_name.clone());
                            self.env.insert(
                                *m_id,
                                Ty::Function(param_tys.clone(), Box::new(ret_ty.clone())),
                            );
                            self.methods_env.insert(
                                method_mangled_name.clone(),
                                Ty::Function(param_tys, Box::new(ret_ty)),
                            );
                            self.global_spans.insert(method_mangled_name, *m_span);
                        }
                    }
                }
                Decl::Enum {
                    id,
                    generic_params,
                    variants,
                    ..
                } => {
                    if generic_params.is_some() {
                        continue;
                    }
                    for v in variants {
                        if let Some(fields) = &v.fields {
                            let mut param_tys = Vec::new();
                            for (_, fty) in fields {
                                let f_ty = self.resolve_type(fty).map_err(|e| {
                                    self.reporter.report(pace_errors::Diagnostic::error(e).with_span(fty.span()));
                                    "Type resolution failed".to_string()
                                })?;
                                param_tys.push(f_ty);
                            }
                            self.env
                                .insert(v.id, Ty::Function(param_tys, Box::new(Ty::Enum(*id))));
                        } else {
                            self.env.insert(v.id, Ty::Enum(*id));
                        }
                    }
                }
                _ => {}
            }
        }

        // Pass 2: Register struct/class fields
        for (module_name, decl) in &declarations {
            self.current_module = Some(module_name.to_string());
            match decl {
                Decl::Struct {
                    id,
                    generic_params,
                    fields,
                    with,
                    ..
                } => {
                    if generic_params.is_some() {
                        continue;
                    }
                    let mut resolved_fields = Vec::new();
                    for pace_hir::HirFieldDef {
                        name: fname,
                        ty: fty,
                        is_mut,
                        is_private,
                        ..
                    } in fields
                    {
                        resolved_fields.push(ResolvedField {
                            name: fname.to_string(),
                            ty: self.resolve_type(fty)?,
                            is_mut: *is_mut,
                            is_private: *is_private,
                        });
                    }
                    self.struct_defs.insert(*id, resolved_fields);
                    self.implemented_traits
                        .insert(*id, with.iter().map(|w| w.to_string()).collect());
                }
                Decl::Class {
                    id,
                    generic_params,
                    fields,
                    with,
                    ..
                }
                | Decl::Actor {
                    id,
                    generic_params,
                    fields,
                    with,
                    ..
                } => {
                    if generic_params.is_some() {
                        continue;
                    }
                    let mut resolved_fields = Vec::new();
                    for pace_hir::HirFieldDef {
                        name: fname,
                        ty: fty,
                        is_mut,
                        is_private,
                        ..
                    } in fields
                    {
                        resolved_fields.push(ResolvedField {
                            name: fname.to_string(),
                            ty: self.resolve_type(fty)?,
                            is_mut: *is_mut,
                            is_private: *is_private,
                        });
                    }
                    self.class_defs.insert(*id, resolved_fields);
                    self.implemented_traits
                        .insert(*id, with.iter().map(|w| w.to_string()).collect());
                }
                Decl::Enum {
                    id,
                    generic_params,
                    variants,
                    ..
                } => {
                    if generic_params.is_some() {
                        continue;
                    }
                    self.enum_defs.insert(*id, variants.clone());
                }
                _ => {}
            }
        }

        // Pass 2.5: Hierarchy Resolution & Field Inheritance
        for (module_name, decl) in &declarations {
            self.current_module = Some(module_name.to_string());
            if let Decl::Class {
                id, extends, span, ..
            } = decl
                && let Some(parent_name) = extends
            {
                let mangled_parent_name = if let Some(scope) = self.module_scopes.get(module_name) {
                    scope
                        .visible_symbols
                        .get(&parent_name.to_string())
                        .cloned()
                        .unwrap_or_else(|| parent_name.to_string())
                } else {
                    parent_name.to_string()
                };
                if let Some(&(parent_id, 1)) = self.named_types.get(&mangled_parent_name) {
                    if parent_id == *id {
                        self.reporter.report(
                            pace_errors::Diagnostic::error("Class cannot inherit from itself")
                                .with_span(*span),
                        );
                        return Err("Class cannot inherit from itself".to_string());
                    }
                    self.class_parents.insert(*id, parent_id);
                } else {
                    self.reporter.report(
                        pace_errors::Diagnostic::error("Class extends a non-class or unknown type")
                            .with_span(*span),
                    );
                    return Err("Class extends a non-class or unknown type".to_string());
                }
            }
        }

        let class_ids: Vec<HirId> = self.class_defs.keys().copied().collect();
        let original_class_defs = self.class_defs.clone();
        for &id in &class_ids {
            let mut all_fields = Vec::new();
            let mut curr = Some(id);
            let mut hierarchy = Vec::new();

            while let Some(c_id) = curr {
                if hierarchy.contains(&c_id) {
                    self.reporter.report(
                        pace_errors::Diagnostic::error("Circular inheritance detected")
                            .with_span(pace_span::Span::new(pace_span::FileId::DUMMY, 0, 0)),
                    );
                    return Err("Circular inheritance detected".to_string());
                }
                hierarchy.push(c_id);
                curr = self.class_parents.get(&c_id).copied();
            }

            for &c_id in hierarchy.iter().rev() {
                if let Some(fields) = original_class_defs.get(&c_id) {
                    all_fields.extend(fields.clone());
                }
            }
            self.class_defs.insert(id, all_fields);
        }

        // Pass 3: Register functions
        for (module_name, decl) in &declarations {
            self.current_module = Some(module_name.to_string());
            if let Decl::Function {
                id: func_id,
                name,
                is_private,
                generic_params,
                params,
                return_type,
                span,
                ..
            } = decl
            {
                if *is_private {
                    if let Some(scope) = self.module_scopes.get_mut(module_name) {
                        scope.private_symbols.insert(name.to_string());
                    }
                }
                if generic_params.is_some() {
                    continue;
                }
                let mut param_tys = Vec::new();
                for (_, _, pty) in params {
                    param_tys.push(self.resolve_type(pty)?);
                }
                let ret_ty = if let Some(rty) = return_type {
                    self.resolve_type(rty)?
                } else {
                    Ty::Void
                };
                let mangled_name = if name == "main" {
                    name.to_string()
                } else {
                    format!("{}_{}", module_name, name)
                };
                self.global_functions.insert(
                    mangled_name.clone(),
                    Ty::Function(param_tys, Box::new(ret_ty)),
                );
                self.global_spans.insert(mangled_name.clone(), *span);
                self.resolved_global_names
                    .insert(*func_id, mangled_name.clone());

                if let Some(scope) = self.module_scopes.get_mut(module_name) {
                    scope.visible_symbols.insert(name.to_string(), mangled_name);
                }
            } else if let Decl::Let {
                id,
                name,
                is_private,
                span,
                ..
            }
            | Decl::Var {
                id,
                name,
                is_private,
                span,
                ..
            }
            | Decl::Const {
                id,
                name,
                is_private,
                span,
                ..
            } = decl
            {
                if *is_private {
                    if let Some(scope) = self.module_scopes.get_mut(module_name) {
                        scope.private_symbols.insert(name.to_string());
                    }
                }
                let mangled_name = format!("{}_{}", module_name, name);
                self.global_spans.insert(mangled_name.clone(), *span);
                self.resolved_global_names.insert(*id, mangled_name.clone());
                if let Some(scope) = self.module_scopes.get_mut(module_name) {
                    scope.visible_symbols.insert(name.to_string(), mangled_name);
                }
            }
        }

        // Resolve wildcard imports
        let mut wildcard_symbols_to_add: HashMap<String, Vec<(String, String)>> = HashMap::new();
        for module_name in &program.module_order {
            let scope = match self.module_scopes.get(&module_name.to_string()) {
                Some(s) => s,
                None => return Err(format!("ICE: Module scope {} not found", module_name)),
            };
            let mut symbols_to_add = Vec::new();
            for target_module in &scope.wildcard_imports {
                if let Some(target_scope) = self.module_scopes.get(target_module) {
                    for (symbol_name, mangled_name) in &target_scope.visible_symbols {
                        if !target_scope.private_symbols.contains(symbol_name)
                            && !scope.visible_symbols.contains_key(symbol_name)
                        {
                            symbols_to_add.push((symbol_name.clone(), mangled_name.clone()));
                        }
                    }
                }
            }
            wildcard_symbols_to_add.insert(module_name.to_string(), symbols_to_add);
        }
        for module_name in &program.module_order {
            if let Some(symbols) = wildcard_symbols_to_add.remove(&module_name.to_string())
                && let Some(scope) = self.module_scopes.get_mut(&module_name.to_string())
            {
                for (name, mangled) in symbols {
                    scope.visible_symbols.insert(name, mangled);
                }
            }
        }

        // Pass 3.5: Construct V-Tables and Validate Overrides
        let mut class_methods: HashMap<HirId, Vec<ResolvedMethod>> = HashMap::new();
        for (module_name, decl) in &declarations {
            self.current_module = Some(module_name.to_string());
            if let Decl::Class {
                id,
                name: decl_name,
                methods,
                ..
            } = decl
            {
                let mut cm = Vec::new();
                for m in methods {
                    if let Decl::Function {
                        name,
                        annotations,
                        is_private,
                        span,
                        ..
                    } = m
                    {
                        let mangled_class_name = format!("{}_{}", module_name, decl_name);
                        let class_prefix = format!("{}_", decl_name);
                        let short_method_name = name.trim_prefix(&class_prefix);
                        let method_mangled_name =
                            format!("{}_{}", mangled_class_name, short_method_name);

                        if let Some(ty) = self.methods_env.get(&method_mangled_name) {
                            let base_name = short_method_name.to_string();
                            cm.push(ResolvedMethod {
                                name: base_name,
                                ty: ty.clone(),
                                mangled_name: method_mangled_name,
                                is_override: annotations.iter().any(|a| a.name.as_str() == "override"),
                                is_private: *is_private,
                                span: *span,
                            });
                        }
                    }
                }
                class_methods.insert(*id, cm);
            }
        }

        for &id in &class_ids {
            let mut vtable: Vec<VTableEntry> = Vec::new();
            let mut hierarchy = Vec::new();
            let mut curr = Some(id);
            while let Some(c_id) = curr {
                hierarchy.push(c_id);
                curr = self.class_parents.get(&c_id).copied();
            }

            for &c_id in hierarchy.iter().rev() {
                if let Some(methods) = class_methods.get(&c_id) {
                    for ResolvedMethod {
                        name: base_name,
                        ty,
                        mangled_name: full_name,
                        is_override,
                        span,
                        ..
                    } in methods
                    {
                        if *is_override {
                            if let Some(pos) = vtable.iter().position(|v| &v.name == base_name) {
                                // Validate signature (ignoring the first `self` parameter's exact type, but checking length and other params)
                                let base_ty = &vtable[pos].ty;
                                let mut sig_match = false;
                                if let (
                                    Ty::Function(base_params, base_ret),
                                    Ty::Function(new_params, new_ret),
                                ) = (base_ty, ty)
                                    && *base_ret == *new_ret
                                    && base_params.len() == new_params.len()
                                {
                                    let mut params_match = true;
                                    for i in 1..base_params.len() {
                                        if base_params[i] != new_params[i] {
                                            params_match = false;
                                            break;
                                        }
                                    }
                                    if params_match {
                                        sig_match = true;
                                    }
                                }

                                if !sig_match {
                                    self.reporter.report(pace_errors::Diagnostic::error(format!("Method '{}' overrides parent method but has a different signature", base_name)).with_span(*span));
                                    return Err("Signature mismatch in override".to_string());
                                }
                                vtable[pos] = VTableEntry {
                                    name: base_name.to_string(),
                                    ty: ty.clone(),
                                    mangled_name: full_name.clone(),
                                };
                            } else {
                                self.reporter.report(pace_errors::Diagnostic::error(format!("Method '{}' marked as override but does not override any parent method", base_name)).with_span(*span));
                                return Err("Invalid override".to_string());
                            }
                        } else {
                            if vtable.iter().any(|v| &v.name == base_name) && base_name != "init" {
                                self.reporter.report(pace_errors::Diagnostic::error(format!("Method '{}' shadows a parent method. Use '@override' annotation", base_name)).with_span(*span));
                                return Err("Missing override keyword".to_string());
                            }
                            vtable.push(VTableEntry {
                                name: base_name.to_string(),
                                ty: ty.clone(),
                                mangled_name: full_name.clone(),
                            });
                        }
                    }
                }
            }
            self.class_vtables.insert(id, vtable);
        }

        for (module_name, decl) in &declarations {
            self.current_module = Some(module_name.to_string());
            self.check_decl(decl)?;
        }

        // Pass 4: Unused Variables and Functions Linter Sweep
        for (id, name, span) in &self.declared_bindings {
            let is_compiler_generated = self.is_compiler_generated_name(*id, name);
            if !self.used_bindings.contains(id)
                && !self.used_bindings_by_name.contains(name)
                && !name.starts_with('_')
                && name != "main"
                && name != "self"
                && !name.ends_with("_init")
                && !is_compiler_generated
            {
                self.reporter.report(
                    Diagnostic::warning(format!("unused variable or function: `{}`", name))
                        .with_span(*span)
                        .with_code(ErrorCode::UnusedVariable)
                        .with_hint(format!(
                            "if this is intentional, prefix it with an underscore: `_{}`",
                            name
                        )),
                );
            }
        }

        Ok(())
    }
}
