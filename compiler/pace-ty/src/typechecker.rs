use pace_hir::{Decl, Expr, HirId, Program, Stmt};
use std::collections::HashMap;

use crate::ty::Ty;

use pace_errors::{Diagnostic, ErrorCode, Reporter};

pub struct TypeChecker {
    pub env: HashMap<HirId, Ty>,
    pub mutability_env: HashMap<HirId, bool>, // true = mutable, false = immutable
    pub loop_depth: usize,
    pub next_id: u32,
    pub methods_env: HashMap<String, Ty>,
    pub global_functions: HashMap<String, Ty>,
    pub struct_defs: HashMap<HirId, Vec<(String, Ty, bool)>>,
    pub class_defs: HashMap<HirId, Vec<(String, Ty, bool)>>,
    pub class_parents: HashMap<HirId, HirId>,
    pub class_vtables: HashMap<HirId, Vec<(String, Ty, String)>>,
    pub enum_defs: HashMap<HirId, Vec<pace_hir::EnumVariant>>,
    pub trait_defs: HashMap<String, Decl>,
    pub static_fields_env: HashMap<String, Ty>, // format: "{class_name}_{field_name}"
    pub const_env: HashMap<String, Ty>,
    pub named_types: HashMap<String, (HirId, u8)>, // maps name like "User" to (HirId, 0=struct, 1=class, 2=enum)
    pub generic_templates: HashMap<String, Decl>,
    pub generic_templates_by_id: HashMap<HirId, String>,
    pub current_expected_ty: Option<Ty>,
    pub current_fn_name: Option<String>,
    pub declared_bindings: Vec<(HirId, String, pace_span::Span)>,
    pub used_bindings: std::collections::HashSet<HirId>,
    pub initialized_bindings: std::collections::HashSet<HirId>,
    pub local_types: HashMap<HirId, Ty>, // Persisted types of all variables
    pub reporter: Reporter,
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            env: HashMap::new(),
            mutability_env: HashMap::new(),
            methods_env: HashMap::new(),
            global_functions: HashMap::new(),
            struct_defs: HashMap::new(),
            class_defs: HashMap::new(),
            class_parents: HashMap::new(),
            class_vtables: HashMap::new(),
            enum_defs: HashMap::new(),
            trait_defs: HashMap::new(),
            static_fields_env: HashMap::new(),
            const_env: HashMap::new(),
            named_types: HashMap::new(),
            generic_templates: HashMap::new(),
            generic_templates_by_id: HashMap::new(),
            current_expected_ty: None,
            current_fn_name: None,
            declared_bindings: Vec::new(),
            used_bindings: std::collections::HashSet::new(),
            initialized_bindings: std::collections::HashSet::new(),
            local_types: HashMap::new(),
            loop_depth: 0,
            next_id: 1000000,
            reporter: Reporter::new(),
        }
    }

    pub fn generate_id(&mut self) -> pace_hir::HirId {
        let id = self.next_id;
        self.next_id += 1;
        pace_hir::HirId(id)
    }

    pub fn get_type(&self, ast_ty: &pace_ast::Type) -> Result<Ty, String> {
        match ast_ty {
            pace_ast::Type::Named(id) => match id.name.as_str() {
                "Int" | "int" => Ok(Ty::Int),
                "Float" | "float" => Ok(Ty::Float),
                "String" | "string" => Ok(Ty::String),
                "Bool" | "bool" => Ok(Ty::Bool),
                "Void" | "void" => Ok(Ty::Void),
                other => {
                    if let Some(&(hir_id, kind)) = self.named_types.get(other) {
                        if kind == 1 {
                            return Ok(Ty::Class(hir_id));
                        } else if kind == 0 {
                            return Ok(Ty::Struct(hir_id));
                        } else {
                            return Ok(Ty::Enum(hir_id));
                        }
                    }
                    Err(format!("Unknown type: {}", other))
                }
            },
            pace_ast::Type::Generic(base, args, _) => {
                if let pace_ast::Type::Named(id) = &**base {
                    let mut final_args = Vec::new();
                    if let Some(template) = self.generic_templates.get(&id.name) {
                        let generic_params = match template {
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
                            _ => vec![],
                        };
                        for (i, (_, default_ty)) in generic_params.iter().enumerate() {
                            if i < args.len() {
                                final_args.push(args[i].clone());
                            } else if let Some(def_ty) = default_ty {
                                final_args.push(def_ty.clone());
                            }
                        }
                    } else {
                        final_args = args.clone();
                    }

                    let mut mono_name = id.name.to_string();
                    for arg in final_args {
                        let arg_ty = self.get_type(&arg)?;
                        mono_name.push_str(
                            &format!("_{:?}", arg_ty)
                                .replace(" ", "")
                                .replace("(", "_")
                                .replace(")", "_")
                                .replace(":", "_"),
                        );
                    }
                    if let Some(&(hir_id, kind)) = self.named_types.get(&mono_name) {
                        if kind == 1 {
                            return Ok(Ty::Class(hir_id));
                        } else if kind == 0 {
                            return Ok(Ty::Struct(hir_id));
                        } else {
                            return Ok(Ty::Enum(hir_id));
                        }
                    }
                    Err(format!("Generic instantiation not found: {}", mono_name))
                } else {
                    Err("Complex generic base types not supported".to_string())
                }
            }
            pace_ast::Type::Optional(inner_ty, _) => {
                let inner = self.get_type(inner_ty)?;
                Ok(Ty::Optional(Box::new(inner)))
            }
        }
    }

    pub fn resolve_type(&mut self, ast_ty: &pace_ast::Type) -> Result<Ty, String> {
        match ast_ty {
            pace_ast::Type::Named(id) => {
                match id.name.as_str() {
                    "Int" | "int" => Ok(Ty::Int),
                    "Float" | "float" => Ok(Ty::Float),
                    "String" | "string" => Ok(Ty::String),
                    "Bool" | "bool" => Ok(Ty::Bool),
                    "Void" | "void" => Ok(Ty::Void),
                    other => {
                        if let Some(&(hir_id, kind)) = self.named_types.get(other) {
                            if kind == 1 {
                                return Ok(Ty::Class(hir_id));
                            } else if kind == 0 {
                                return Ok(Ty::Struct(hir_id));
                            } else {
                                return Ok(Ty::Enum(hir_id));
                            }
                        }
                        // Check if it's a generic template being used without arguments
                        if self.generic_templates.contains_key(other) {
                            return Err(format!("Type {} requires generic arguments", other));
                        }
                        Err(format!("Unknown type: {}", other))
                    }
                }
            }
            pace_ast::Type::Generic(base, args, _) => {
                if let pace_ast::Type::Named(id) = &**base {
                    self.instantiate_generic(&id.name, args)
                } else {
                    Err("Complex generic base types not supported".to_string())
                }
            }
            pace_ast::Type::Optional(inner_ty, _) => {
                let inner = self.resolve_type(inner_ty)?;
                Ok(Ty::Optional(Box::new(inner)))
            }
        }
    }

    pub fn check_program(&mut self, program: &Program) -> Result<(), String> {
        let mut has_main = false;

        for decl in &program.declarations {
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
        if !has_main {
            self.reporter.report(
                Diagnostic::warning("No 'main' function found")
                    .with_hint("Executables must have an entry point 'fn main()'"),
            );
        }

        // 1. Gather all top-level types (Structs/Classes/Functions)
        for decl in &program.declarations {
            match decl {
                Decl::Struct {
                    id,
                    name,
                    generic_params,
                    static_fields,
                    const_fields,
                    methods,
                    ..
                } => {
                    if generic_params.is_some() {
                        self.generic_templates.insert(name.clone(), decl.clone());
                        self.generic_templates_by_id.insert(*id, name.clone());
                        continue;
                    }
                    self.named_types.insert(name.clone(), (*id, 0));
                    for (sf_name, sf_ty, _) in static_fields {
                        let resolved_ty = self.resolve_type(sf_ty).unwrap_or(Ty::Int);
                        self.static_fields_env
                            .insert(format!("{}_{}", name, sf_name), resolved_ty);
                    }
                    for (cf_name, cf_ty, _) in const_fields {
                        let resolved_ty = self.resolve_type(cf_ty).unwrap_or(Ty::Int);
                        self.const_env
                            .insert(format!("{}_{}", name, cf_name), resolved_ty);
                    }
                    for method in methods {
                        if let Decl::Function {
                            name: m_name,
                            params,
                            return_type,
                            id: m_id,
                            ..
                        } = method
                        {
                            let mut param_tys = Vec::new();
                            for (_, _, pty) in params {
                                param_tys.push(self.resolve_type(pty).unwrap_or(Ty::Int));
                            }
                            let ret_ty = if let Some(r) = return_type {
                                self.resolve_type(r).unwrap_or(Ty::Void)
                            } else {
                                Ty::Void
                            };
                            self.env.insert(
                                *m_id,
                                Ty::Function(param_tys.clone(), Box::new(ret_ty.clone())),
                            );
                            self.methods_env
                                .insert(m_name.clone(), Ty::Function(param_tys, Box::new(ret_ty)));
                        }
                    }
                }
                Decl::Class {
                    id,
                    name,
                    generic_params,
                    static_fields,
                    const_fields,
                    methods,
                    ..
                } => {
                    if generic_params.is_some() {
                        self.generic_templates.insert(name.clone(), decl.clone());
                        self.generic_templates_by_id.insert(*id, name.clone());
                        continue;
                    }
                    self.named_types.insert(name.clone(), (*id, 1));
                    for (sf_name, sf_ty, _) in static_fields {
                        let resolved_ty = self.resolve_type(sf_ty).unwrap_or(Ty::Int);
                        self.static_fields_env
                            .insert(format!("{}_{}", name, sf_name), resolved_ty);
                    }
                    for (cf_name, cf_ty, _) in const_fields {
                        let resolved_ty = self.resolve_type(cf_ty).unwrap_or(Ty::Int);
                        self.const_env
                            .insert(format!("{}_{}", name, cf_name), resolved_ty);
                    }
                    for method in methods {
                        if let Decl::Function {
                            name: m_name,
                            params,
                            return_type,
                            id: m_id,
                            ..
                        } = method
                        {
                            let mut param_tys = Vec::new();
                            for (_, _, pty) in params {
                                param_tys.push(self.resolve_type(pty).unwrap_or(Ty::Int));
                            }
                            let ret_ty = if let Some(r) = return_type {
                                self.resolve_type(r).unwrap_or(Ty::Void)
                            } else {
                                Ty::Void
                            };
                            self.env.insert(
                                *m_id,
                                Ty::Function(param_tys.clone(), Box::new(ret_ty.clone())),
                            );
                            self.methods_env
                                .insert(m_name.clone(), Ty::Function(param_tys, Box::new(ret_ty)));
                        }
                    }
                }
                Decl::Trait { name, .. } => {
                    self.trait_defs.insert(name.clone(), decl.clone());
                }
                Decl::Enum {
                    id,
                    name,
                    generic_params,
                    variants,
                    ..
                } => {
                    if generic_params.is_some() {
                        self.generic_templates.insert(name.clone(), decl.clone());
                        self.generic_templates_by_id.insert(*id, name.clone());
                        continue;
                    }
                    self.named_types.insert(name.clone(), (*id, 2));
                    for v in variants {
                        if let Some(fields) = &v.fields {
                            let mut param_tys = Vec::new();
                            for (_, fty) in fields {
                                param_tys.push(self.resolve_type(fty).unwrap_or(Ty::Int));
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
        for decl in &program.declarations {
            match decl {
                Decl::Struct {
                    id,
                    generic_params,
                    fields,
                    ..
                } => {
                    if generic_params.is_some() {
                        continue;
                    }
                    let mut resolved_fields = Vec::new();
                    for (fname, fty, _, is_mut) in fields {
                        resolved_fields.push((fname.clone(), self.resolve_type(fty)?, *is_mut));
                    }
                    self.struct_defs.insert(*id, resolved_fields);
                }
                Decl::Class {
                    id,
                    generic_params,
                    fields,
                    ..
                } => {
                    if generic_params.is_some() {
                        continue;
                    }
                    let mut resolved_fields = Vec::new();
                    for (fname, fty, _, is_mut) in fields {
                        resolved_fields.push((fname.clone(), self.resolve_type(fty)?, *is_mut));
                    }
                    self.class_defs.insert(*id, resolved_fields);
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
        for decl in &program.declarations {
            if let Decl::Class {
                id, extends, span, ..
            } = decl
            {
                if let Some(parent_name) = extends {
                    if let Some(&(parent_id, 1)) = self.named_types.get(parent_name) {
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
                            pace_errors::Diagnostic::error(
                                "Class extends a non-class or unknown type",
                            )
                            .with_span(*span),
                        );
                        return Err("Class extends a non-class or unknown type".to_string());
                    }
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
                            .with_span(pace_span::Span::new(0, 0)),
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
        for decl in &program.declarations {
            if let Decl::Function {
                id,
                name,
                generic_params,
                params,
                return_type,
                ..
            } = decl
            {
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
                self.env
                    .insert(*id, Ty::Function(param_tys.clone(), Box::new(ret_ty.clone())));
                self.global_functions.insert(name.clone(), Ty::Function(param_tys, Box::new(ret_ty)));
            }
        }

        // Pass 3.5: Construct V-Tables and Validate Overrides
        let mut class_methods: HashMap<HirId, Vec<(String, Ty, String, bool, pace_span::Span)>> =
            HashMap::new();
        for decl in &program.declarations {
            if let Decl::Class { id, methods, .. } = decl {
                let mut cm = Vec::new();
                for m in methods {
                    if let Decl::Function {
                        name,
                        is_override,
                        span,
                        ..
                    } = m
                    {
                        if let Some(ty) = self.methods_env.get(name) {
                            let base_name = name.split('_').last().unwrap().to_string();
                            cm.push((base_name, ty.clone(), name.clone(), *is_override, *span));
                        }
                    }
                }
                class_methods.insert(*id, cm);
            }
        }

        for &id in &class_ids {
            let mut vtable: Vec<(String, Ty, String)> = Vec::new();
            let mut hierarchy = Vec::new();
            let mut curr = Some(id);
            while let Some(c_id) = curr {
                hierarchy.push(c_id);
                curr = self.class_parents.get(&c_id).copied();
            }

            for &c_id in hierarchy.iter().rev() {
                if let Some(methods) = class_methods.get(&c_id) {
                    for (base_name, ty, full_name, is_override, span) in methods {
                        if *is_override {
                            if let Some(pos) = vtable.iter().position(|(n, _, _)| n == base_name) {
                                // Validate signature (ignoring the first `self` parameter's exact type, but checking length and other params)
                                let base_ty = &vtable[pos].1;
                                let mut sig_match = false;
                                if let (
                                    Ty::Function(base_params, base_ret),
                                    Ty::Function(new_params, new_ret),
                                ) = (base_ty, ty)
                                {
                                    if base_ret == new_ret && base_params.len() == new_params.len()
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
                                }

                                if !sig_match {
                                    self.reporter.report(pace_errors::Diagnostic::error(format!("Method '{}' overrides parent method but has a different signature", base_name)).with_span(*span));
                                    return Err(format!("Signature mismatch in override"));
                                }
                                vtable[pos] = (base_name.clone(), ty.clone(), full_name.clone());
                            } else {
                                self.reporter.report(pace_errors::Diagnostic::error(format!("Method '{}' marked as override but does not override any parent method", base_name)).with_span(*span));
                                return Err(format!("Invalid override"));
                            }
                        } else {
                            if vtable.iter().any(|(n, _, _)| n == base_name) {
                                if base_name != "init" {
                                    self.reporter.report(pace_errors::Diagnostic::error(format!("Method '{}' shadows a parent method. Use 'override' keyword", base_name)).with_span(*span));
                                    return Err(format!("Missing override keyword"));
                                }
                            }
                            vtable.push((base_name.clone(), ty.clone(), full_name.clone()));
                        }
                    }
                }
            }
            self.class_vtables.insert(id, vtable);
        }

        for decl in &program.declarations {
            self.check_decl(decl)?;
        }

        // Pass 4: Unused Variables and Functions Linter Sweep
        for (id, name, span) in &self.declared_bindings {
            let is_compiler_generated = self
                .named_types
                .keys()
                .any(|k| name.starts_with(&format!("{}_", k)));
            if !self.used_bindings.contains(&id)
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

    pub fn check_block(
        &mut self,
        block: &pace_hir::Block,
        expected_ret_ty: Option<&Ty>,
    ) -> Result<(), String> {
        let outer_env = self.env.clone();
        for stmt in &block.statements {
            match stmt {
                pace_hir::Stmt::Let {
                    id,
                    name,
                    ty: explicit_ty,
                    value,
                    span,
                } => {
                    let mut expected_ty = None;
                    if let Some(explicit) = explicit_ty {
                        expected_ty = Some(self.resolve_type(explicit)?);
                    }

                    let mut ty = if let Some(val) = value {
                        self.initialized_bindings.insert(*id);
                        let prev_expected = self.current_expected_ty.take();
                        self.current_expected_ty = expected_ty.clone();
                        let res = self.check_expr(val);
                        self.current_expected_ty = prev_expected;
                        res?
                    } else {
                        // Uninitialized variable
                        let expected = expected_ty.clone().unwrap();
                        if let Ty::Optional(_) = expected {
                            self.initialized_bindings.insert(*id); // Implicitly initialized to null
                        }
                        expected
                    };

                    if let Some(expected) = expected_ty {
                        if ty != expected {
                            if expected != Ty::Optional(Box::new(ty.clone())) {
                                self.reporter.report(
                                    pace_errors::Diagnostic::error(format!(
                                        "Type mismatch: expected {:?}, got {:?}",
                                        expected, ty
                                    ))
                                    .with_span(explicit_ty.as_ref().unwrap().span()),
                                );
                                return Err("Type mismatch".to_string());
                            }
                            ty = expected;
                        }
                    }
                    self.env.insert(*id, ty.clone());
                    self.local_types.insert(*id, ty.clone());
                    self.mutability_env.insert(*id, false); // Let is immutable
                    self.declared_bindings.push((*id, name.clone(), *span));
                }
                pace_hir::Stmt::Var {
                    id,
                    name,
                    ty: explicit_ty,
                    value,
                    span,
                } => {
                    let mut ty = if let Some(val) = value {
                        self.initialized_bindings.insert(*id);
                        self.check_expr(val)?
                    } else {
                        // Uninitialized variable
                        let explicit = explicit_ty.as_ref().unwrap(); // Guaranteed by parser
                        let expected = self.resolve_type(explicit)?;
                        if let Ty::Optional(_) = expected {
                            self.initialized_bindings.insert(*id); // Implicitly initialized to null
                        }
                        expected
                    };

                    if let Some(explicit) = explicit_ty {
                        let expected = self.resolve_type(explicit)?;
                        if ty != expected {
                            if expected != Ty::Optional(Box::new(ty.clone())) {
                                self.reporter.report(
                                    pace_errors::Diagnostic::error(format!(
                                        "Type mismatch: expected {:?}, got {:?}",
                                        expected, ty
                                    ))
                                    .with_span(explicit.span()),
                                );
                                return Err("Type mismatch".to_string());
                            }
                            ty = expected; // Promote to Optional
                        }
                    }
                    self.env.insert(*id, ty.clone());
                    self.local_types.insert(*id, ty.clone());
                    self.mutability_env.insert(*id, true); // Var is mutable
                    self.declared_bindings.push((*id, name.clone(), *span));
                }
                pace_hir::Stmt::ExprStmt(expr, _) => {
                    self.check_expr(expr)?;
                }
                pace_hir::Stmt::Return(expr, span) => {
                    let ret_ty = if let Some(e) = expr {
                        self.check_expr(e)?
                    } else {
                        Ty::Void
                    };

                    if let Some(expected) = expected_ret_ty {
                        if ret_ty != *expected {
                            self.reporter.report(Diagnostic::error(format!("Type mismatch: function expects to return {:?}, but returned {:?}", expected, ret_ty))
                                .with_span(*span)
                                .with_code(ErrorCode::TypeMismatch));
                        }
                    }
                }
            }
        }
        self.env = outer_env;
        Ok(())
    }

    pub fn check_decl(&mut self, decl: &Decl) -> Result<(), String> {
        match decl {
            Decl::Let {
                id,
                name,
                ty: explicit_ty,
                value,
                span,
            } => {
                let mut expected_ty = None;
                if let Some(explicit) = explicit_ty {
                    expected_ty = Some(self.resolve_type(explicit)?);
                }

                let mut ty = if let Some(val) = value {
                    self.initialized_bindings.insert(*id);
                    let prev_expected = self.current_expected_ty.take();
                    self.current_expected_ty = expected_ty.clone();
                    let res = self.check_expr(val);
                    self.current_expected_ty = prev_expected;
                    res?
                } else {
                    let expected = expected_ty.clone().unwrap();
                    if let Ty::Optional(_) = expected {
                        self.initialized_bindings.insert(*id);
                    }
                    expected
                };

                if let Some(expected) = expected_ty {
                    if ty != expected {
                        if expected != Ty::Optional(Box::new(ty.clone())) {
                            self.reporter.report(
                                pace_errors::Diagnostic::error(format!(
                                    "Type mismatch: expected {:?}, got {:?}",
                                    expected, ty
                                ))
                                .with_span(explicit_ty.as_ref().unwrap().span()),
                            );
                            return Err("Type mismatch".to_string());
                        }
                        ty = expected;
                    }
                }
                self.env.insert(*id, ty.clone());
                self.local_types.insert(*id, ty.clone());
                self.mutability_env.insert(*id, false); // Let is immutable
                self.declared_bindings.push((*id, name.clone(), *span));
                Ok(())
            }
            Decl::Var {
                id,
                name,
                ty: explicit_ty,
                value,
                span,
            } => {
                let mut ty = if let Some(val) = value {
                    self.initialized_bindings.insert(*id);
                    self.check_expr(val)?
                } else {
                    let explicit = explicit_ty.as_ref().unwrap();
                    let expected = self.resolve_type(explicit)?;
                    if let Ty::Optional(_) = expected {
                        self.initialized_bindings.insert(*id);
                    }
                    expected
                };

                if let Some(explicit) = explicit_ty {
                    let expected = self.resolve_type(explicit)?;
                    if ty != expected {
                        if expected != Ty::Optional(Box::new(ty.clone())) {
                            self.reporter.report(
                                pace_errors::Diagnostic::error(format!(
                                    "Type mismatch: expected {:?}, got {:?}",
                                    expected, ty
                                ))
                                .with_span(explicit.span()),
                            );
                            return Err("Type mismatch".to_string());
                        }
                        ty = expected;
                    }
                }
                self.env.insert(*id, ty.clone());
                self.local_types.insert(*id, ty.clone());
                self.mutability_env.insert(*id, true); // Var is mutable
                self.declared_bindings.push((*id, name.clone(), *span));
                Ok(())
            }
            Decl::Const {
                id,
                name,
                ty: explicit_ty,
                value,
                span,
            } => {
                let mut ty = self.check_expr(value)?;
                if let Some(explicit) = explicit_ty {
                    let expected = self.resolve_type(explicit)?;
                    if ty != expected {
                        if expected != Ty::Optional(Box::new(ty.clone())) {
                            self.reporter.report(
                                pace_errors::Diagnostic::error(format!(
                                    "Type mismatch: expected {:?}, got {:?}",
                                    expected, ty
                                ))
                                .with_span(explicit.span()),
                            );
                            return Err("Type mismatch".to_string());
                        }
                        ty = expected;
                    }
                }
                self.env.insert(*id, ty);
                self.mutability_env.insert(*id, false);
                self.declared_bindings.push((*id, name.clone(), *span));
                Ok(())
            }
            Decl::Struct {
                id,
                generic_params,
                methods,
                static_fields,
                const_fields,
                ..
            } => {
                if generic_params.is_some() {
                    return Ok(());
                }
                self.env.insert(*id, Ty::Struct(*id));
                for (_, sf_ty, sf_expr) in static_fields {
                    let expected_ty = self.resolve_type(sf_ty)?;
                    let expr_ty = self.check_expr(sf_expr)?;
                    if expected_ty != expr_ty {
                        self.reporter.report(
                            Diagnostic::error(format!(
                                "Type mismatch: expected {:?}, got {:?}",
                                expected_ty, expr_ty
                            ))
                            .with_span(sf_expr.span())
                            .with_code(ErrorCode::TypeMismatch),
                        );
                        return Err("Type mismatch".to_string());
                    }
                }
                for (_, cf_ty, cf_expr) in const_fields {
                    let expected_ty = self.resolve_type(cf_ty)?;
                    let expr_ty = self.check_expr(cf_expr)?;
                    if expected_ty != expr_ty {
                        self.reporter.report(
                            Diagnostic::error(format!(
                                "Type mismatch: expected {:?}, got {:?}",
                                expected_ty, expr_ty
                            ))
                            .with_span(cf_expr.span())
                            .with_code(ErrorCode::TypeMismatch),
                        );
                        return Err("Type mismatch".to_string());
                    }
                }
                for method in methods {
                    self.check_decl(method)?;
                }
                Ok(())
            }
            Decl::Class {
                id,
                generic_params,
                methods,
                static_fields,
                const_fields,
                ..
            } => {
                if generic_params.is_some() {
                    return Ok(());
                }
                self.env.insert(*id, Ty::Class(*id));
                for (_, sf_ty, sf_expr) in static_fields {
                    let expected_ty = self.resolve_type(sf_ty)?;
                    let expr_ty = self.check_expr(sf_expr)?;
                    if expected_ty != expr_ty {
                        self.reporter.report(
                            Diagnostic::error(format!(
                                "Type mismatch: expected {:?}, got {:?}",
                                expected_ty, expr_ty
                            ))
                            .with_span(sf_expr.span())
                            .with_code(ErrorCode::TypeMismatch),
                        );
                        return Err("Type mismatch".to_string());
                    }
                }
                for (_, cf_ty, cf_expr) in const_fields {
                    let expected_ty = self.resolve_type(cf_ty)?;
                    let expr_ty = self.check_expr(cf_expr)?;
                    if expected_ty != expr_ty {
                        self.reporter.report(
                            Diagnostic::error(format!(
                                "Type mismatch: expected {:?}, got {:?}",
                                expected_ty, expr_ty
                            ))
                            .with_span(cf_expr.span())
                            .with_code(ErrorCode::TypeMismatch),
                        );
                        return Err("Type mismatch".to_string());
                    }
                }
                for method in methods {
                    self.check_decl(method)?;
                }
                Ok(())
            }
            Decl::Enum {
                id, generic_params, ..
            } => {
                if generic_params.is_some() {
                    return Ok(());
                }
                self.env.insert(*id, Ty::Enum(*id));
                Ok(())
            }
            Decl::Expr(expr, _) => {
                self.check_expr(expr)?;
                Ok(())
            }
            Decl::Function {
                id,
                name,
                generic_params,
                params,
                return_type,
                body,
                span,
                is_static,
                is_override: _,
                ..
            } => {
                if generic_params.is_some() {
                    self.generic_templates.insert(name.clone(), decl.clone());
                    self.generic_templates_by_id.insert(*id, name.clone());
                    return Ok(());
                }
                if name.contains('_') && name != "main" && !name.ends_with("_init") {
                    // Only warn for functions not generated by the compiler
                    let is_compiler_generated =
                        self.named_types.keys().any(|k| name.starts_with(k));
                    if !is_compiler_generated {
                        let offset = if *is_static { 10 } else { 3 };
                        let name_span = pace_span::Span::new(
                            span.start + offset,
                            span.start + offset + name.len(),
                        );
                        self.reporter.report(
                            Diagnostic::warning(format!(
                                "Function '{}' should use camelCase, not snake_case",
                                name
                            ))
                            .with_span(name_span)
                            .with_code(ErrorCode::SnakeCaseName)
                            .with_hint("Rename to camelCase"),
                        );
                    }
                }
                let offset = if *is_static { 10 } else { 3 };
                let name_span =
                    pace_span::Span::new(span.start + offset, span.start + offset + name.len());
                self.declared_bindings.push((*id, name.clone(), name_span));
                self.initialized_bindings.insert(*id);

                // Definite assignment check for initializers
                if name.ends_with("_init") {
                    let type_name = name.trim_end_matches("_init");
                    let mut assigned_fields = std::collections::HashSet::new();
                    let self_id = params
                        .iter()
                        .find(|(_, name, _)| name == "self")
                        .map(|(id, _, _)| *id);

                    // Simple analysis: collect all `self.field = value` assignments in the top-level block
                    for stmt in &body.statements {
                        if let Stmt::ExprStmt(Expr::Assign { target, .. }, _) = stmt {
                            if let Expr::MemberAccess { object, member, .. } = &**target {
                                if let Expr::Ident(obj_id, _, _) = &**object {
                                    if Some(*obj_id) == self_id {
                                        assigned_fields.insert(member.clone());
                                    }
                                }
                            }
                        }
                    }

                    if let Some(&(hir_id, kind)) = self.named_types.get(type_name) {
                        if kind == 1 {
                            let mut calls_super = false;
                            for stmt in &body.statements {
                                if let Stmt::ExprStmt(Expr::Call { callee, .. }, _) = stmt {
                                    if let Expr::MemberAccess { object, .. } = &**callee {
                                        if matches!(&**object, Expr::Super(_)) {
                                            calls_super = true;
                                        }
                                    }
                                }
                            }
                            if calls_super {
                                if let Some(parent_id) = self.class_parents.get(&hir_id) {
                                    if let Some(parent_fields) = self.class_defs.get(parent_id) {
                                        for (fname, _, _) in parent_fields {
                                            assigned_fields.insert(fname.clone());
                                        }
                                    }
                                }
                            }
                        }

                        let fields = if kind == 1 {
                            self.class_defs.get(&hir_id)
                        } else if kind == 0 {
                            self.struct_defs.get(&hir_id)
                        } else {
                            None
                        };

                        if let Some(fields) = fields {
                            for (fname, _, _) in fields {
                                if !assigned_fields.contains(fname) {
                                    self.reporter.report(Diagnostic::error(format!("Field '{}' must be initialized", fname))
                                        .with_span(*span)
                                        .with_hint(format!("Assign a value to 'self.{}' in the init method, or initialize it inline", fname)));
                                    return Err("Definite assignment error".to_string());
                                }
                            }
                        }
                    }
                }

                let outer_env = self.env.clone();
                let prev_fn = self.current_fn_name.take();
                self.current_fn_name = Some(name.clone());
                for (param_id, param_name, pty) in params {
                    let ty = self.resolve_type(pty)?;
                    self.env.insert(*param_id, ty.clone());
                    self.local_types.insert(*param_id, ty);
                    self.declared_bindings
                        .push((*param_id, param_name.clone(), *span));
                    self.initialized_bindings.insert(*param_id);
                }
                let ret_ty = if let Some(rty) = &return_type {
                    self.resolve_type(rty)?
                } else {
                    Ty::Void
                };

                let returns_exhaustively = self.check_exhaustive_return(body);
                if !returns_exhaustively && ret_ty != Ty::Void {
                    self.reporter.report(Diagnostic::error(format!("Function '{}' expects to return {:?}, but does not exhaustively return a value", name, ret_ty))
                        .with_span(*span)
                        .with_code(ErrorCode::NonExhaustiveReturn));
                }

                self.check_block(body, Some(&ret_ty))?;
                self.current_fn_name = prev_fn;
                self.env = outer_env;
                Ok(())
            }
            Decl::Trait { generic_params, .. } => {
                if generic_params.is_some() {
                    return Ok(());
                }
                // Trait methods are static mixins. They are typechecked
                // when they are injected into a Class or Struct.
                Ok(())
            }
        }
    }

    pub fn check_exhaustive_return(&self, block: &pace_hir::Block) -> bool {
        for stmt in &block.statements {
            match stmt {
                pace_hir::Stmt::Return(..) => return true,
                pace_hir::Stmt::ExprStmt(expr, _) => {
                    if let pace_hir::Expr::If {
                        then_block,
                        else_block,
                        ..
                    } = expr
                    {
                        let then_returns = self.check_exhaustive_return(then_block);
                        let else_returns = else_block
                            .as_ref()
                            .map_or(false, |b| self.check_exhaustive_return(b));
                        if then_returns && else_returns {
                            return true;
                        }
                    }
                }
                _ => {}
            }
        }
        false
    }

    fn check_expr(&mut self, expr: &Expr) -> Result<Ty, String> {
        match expr {
            Expr::IntLiteral(..) => Ok(Ty::Int),
            Expr::FloatLiteral(..) => Ok(Ty::Float),
            Expr::BoolLiteral(..) => Ok(Ty::Bool),
            Expr::StringLiteral(..) => Ok(Ty::String),
            Expr::Super(span) => {
                // Find "self" in current environment
                for (id, ty) in &self.env {
                    if let Some((_, name, _)) = self
                        .declared_bindings
                        .iter()
                        .find(|(b_id, _, _)| b_id == id)
                    {
                        if name == "self" {
                            if let Ty::Class(class_id) = ty {
                                if let Some(&parent_id) = self.class_parents.get(class_id) {
                                    return Ok(Ty::Class(parent_id));
                                } else {
                                    self.reporter.report(
                                        pace_errors::Diagnostic::error(
                                            "Cannot use 'super' in a class with no parent",
                                        )
                                        .with_span(*span),
                                    );
                                    return Err(
                                        "Cannot use 'super' in a class with no parent".to_string()
                                    );
                                }
                            }
                        }
                    }
                }
                self.reporter.report(
                    pace_errors::Diagnostic::error("Cannot use 'super' outside of a class method")
                        .with_span(*span),
                );
                Err("Cannot use 'super' outside of a class method".to_string())
            }
            Expr::Ident(id, name, span) => {
                self.used_bindings.insert(*id);
                if !self.initialized_bindings.contains(id) {
                    if let Some((_, name, _)) = self
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
                }
                if let Some(ty) = self.env.get(id).cloned() {
                    Ok(ty)
                } else if let Some(ty) = self.global_functions.get(name).cloned() {
                    Ok(ty)
                } else if let Some(&(hir_id, kind)) = self.named_types.get(name) {
                    if kind == 2 {
                        Ok(Ty::Enum(hir_id))
                    } else {
                        self.env
                            .get(&hir_id)
                            .cloned()
                            .ok_or(format!("Constructor not found for {}", name))
                    }
                } else if let Some(template_name) = self.generic_templates_by_id.get(id) {
                    if let Some(expected) = &self.current_expected_ty {
                        match expected {
                            Ty::Struct(hir_id) | Ty::Class(hir_id) => {
                                if let Some(constructor_ty) = self.env.get(hir_id).cloned() {
                                    Ok(constructor_ty)
                                } else {
                                    Err(format!(
                                        "Constructor not found for instantiated generic '{}'",
                                        template_name
                                    ))
                                }
                            }
                            _ => Ok(expected.clone()),
                        }
                    } else {
                        Err(format!("Cannot infer type for generic template '{}'", name))
                    }
                } else {
                    let err_msg = format!("Cannot find value '{}' in this scope", name);
                    self.reporter.report(Diagnostic::error(&err_msg).with_span(*span));
                    Err(err_msg)
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
                    | pace_ast::BinaryOp::Div => {
                        if left_ty == Ty::Int && right_ty == Ty::Int {
                            Ok(Ty::Int)
                        } else if left_ty == Ty::Float && right_ty == Ty::Float {
                            Ok(Ty::Float)
                        } else {
                            Err(format!(
                                "Type mismatch in binary operation: {:?} and {:?}",
                                left_ty, right_ty
                            ))
                        }
                    }
                    pace_ast::BinaryOp::EqEq
                    | pace_ast::BinaryOp::NotEq
                    | pace_ast::BinaryOp::Gt
                    | pace_ast::BinaryOp::Lt
                    | pace_ast::BinaryOp::GtEq
                    | pace_ast::BinaryOp::LtEq => {
                        Ok(Ty::Int) // Boolean represented as Int in MVP
                    }
                }
            }
            Expr::MemberAccess { object, member, .. } => {
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
                        for (fname, fty, _) in fields {
                            if fname == member {
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
                            return Ok(mty.clone());
                        }

                        Err(format!("Struct has no member '{}'", member))
                    }
                    Ty::Class(hir_id) => {
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
                            .ok_or("Class definition not found")?;
                        for (fname, fty, _) in fields {
                            if fname == member {
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

                        // Check if it's a method
                        let method_name = format!("{}_{}", class_name, member);
                        if let Some(mty) = self.methods_env.get(&method_name) {
                            return Ok(mty.clone());
                        }

                        Err(format!("Class has no member '{}'", member))
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
                                        param_tys.push(self.resolve_type(fty).unwrap_or(Ty::Int));
                                    }
                                    return Ok(Ty::Function(param_tys, Box::new(Ty::Enum(hir_id))));
                                } else {
                                    return Ok(Ty::Enum(hir_id));
                                }
                            }
                        }
                        Err(format!("Enum has no variant '{}'", member))
                    }
                    _ => Err(format!(
                        "Cannot access member '{}' on type {:?}",
                        member, obj_ty
                    )),
                }
            }
            Expr::Call { callee, args, span } => {
                let mut generic_instantiation = None;
                if let Expr::Ident(id, name, _) = &**callee {
                    if self.generic_templates_by_id.contains_key(id) {
                        let template = self.generic_templates.get(name).unwrap().clone();
                        if let pace_hir::Decl::Function {
                            generic_params,
                            params,
                            ..
                        } = template
                        {
                            let generic_params = generic_params.unwrap_or_default();
                            let mut inferred_args = std::collections::HashMap::new();
                            for (i, (_, arg_expr)) in args.iter().enumerate() {
                                if i < params.len() {
                                    let arg_ty = self.check_expr(arg_expr)?;
                                    let (_, _, param_ty) = &params[i];
                                    if let pace_ast::Type::Named(ident) = param_ty {
                                        if generic_params.iter().any(|(p, _)| p == &ident.name) {
                                            inferred_args.insert(ident.name.clone(), arg_ty);
                                        }
                                    }
                                }
                            }

                            let mut ast_args = Vec::new();
                            for (param_name, _) in &generic_params {
                                if let Some(ty) = inferred_args.get(param_name) {
                                    let ty_str = format!("{:?}", ty)
                                        .replace(" ", "")
                                        .replace("(", "_")
                                        .replace(")", "_")
                                        .replace(":", "_");
                                    ast_args.push(pace_ast::Type::Named(pace_ast::Ident {
                                        name: ty_str,
                                        span: *span,
                                    }));
                                }
                            }

                            if ast_args.len() == generic_params.len() {
                                if let Ok(func_ty) = self.instantiate_generic(name, &ast_args) {
                                    generic_instantiation = Some(func_ty);
                                }
                            }
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
                        for (i, (_, fexpr)) in args.iter().enumerate() {
                            let fty = self.check_expr(fexpr)?;
                            if fty != param_tys[i + 1] {
                                self.reporter.report(
                                    Diagnostic::error(format!(
                                        "Argument {} expects type {:?}, got {:?}",
                                        i,
                                        param_tys[i + 1],
                                        fty
                                    ))
                                    .with_span(*span)
                                    .with_code(ErrorCode::TypeMismatch),
                                );
                                return Err(format!(
                                    "Argument {} expects type {:?}, got {:?}",
                                    i,
                                    param_tys[i + 1],
                                    fty
                                ));
                            }
                        }
                        return Ok(Ty::Struct(id));
                    }

                    let def_fields = self.struct_defs.get(&id).unwrap().clone();
                    let mut def_map: std::collections::HashMap<_, _> =
                        def_fields.into_iter().map(|(n, t, _)| (n, t)).collect();
                    for (label, fexpr) in args {
                        let fty = self.check_expr(fexpr)?;
                        if let Some(fname) = label {
                            if let Some(expected_ty) = def_map.remove(fname) {
                                if fty != expected_ty {
                                    self.reporter.report(
                                        Diagnostic::error(format!(
                                            "Field '{}' expects type {:?}, got {:?}",
                                            fname, expected_ty, fty
                                        ))
                                        .with_span(*span)
                                        .with_code(ErrorCode::TypeMismatch),
                                    );
                                    return Err(format!(
                                        "Field '{}' expects type {:?}, got {:?}",
                                        fname, expected_ty, fty
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

                if let Ty::Class(id) = callee_ty {
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
                        for (i, (_, fexpr)) in args.iter().enumerate() {
                            let fty = self.check_expr(fexpr)?;
                            if fty != param_tys[i + 1] {
                                self.reporter.report(
                                    Diagnostic::error(format!(
                                        "Argument {} expects type {:?}, got {:?}",
                                        i,
                                        param_tys[i + 1],
                                        fty
                                    ))
                                    .with_span(*span)
                                    .with_code(ErrorCode::TypeMismatch),
                                );
                                return Err(format!(
                                    "Argument {} expects type {:?}, got {:?}",
                                    i,
                                    param_tys[i + 1],
                                    fty
                                ));
                            }
                        }
                        return Ok(Ty::Class(id));
                    }

                    let def_fields = self.class_defs.get(&id).unwrap().clone();
                    let mut def_map: std::collections::HashMap<_, _> =
                        def_fields.into_iter().map(|(n, t, _)| (n, t)).collect();
                    for (label, fexpr) in args {
                        let fty = self.check_expr(fexpr)?;
                        if let Some(fname) = label {
                            if let Some(expected_ty) = def_map.remove(fname) {
                                if fty != expected_ty {
                                    self.reporter.report(
                                        Diagnostic::error(format!(
                                            "Field '{}' expects type {:?}, got {:?}",
                                            fname, expected_ty, fty
                                        ))
                                        .with_span(*span)
                                        .with_code(ErrorCode::TypeMismatch),
                                    );
                                    return Err(format!(
                                        "Field '{}' expects type {:?}, got {:?}",
                                        fname, expected_ty, fty
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
                    return Ok(Ty::Class(id));
                }

                if let Ty::Function(_, ret_ty) = &callee_ty {
                    if let Ty::Enum(enum_id) = **ret_ty {
                        // Enum variant instantiation!
                        let variants = self.enum_defs.get(&enum_id).unwrap();
                        let mut variant_name = "";
                        if let Expr::MemberAccess { member, .. } = &**callee {
                            variant_name = member;
                        }
                        let mut def_fields = Vec::new();
                        for v in variants {
                            if v.name == variant_name {
                                if let Some(f) = &v.fields {
                                    def_fields = f.clone();
                                }
                            }
                        }
                        let mut def_map: std::collections::HashMap<_, _> = def_fields
                            .into_iter()
                            .map(|(n, t)| (n, self.resolve_type(&t).unwrap_or(Ty::Int)))
                            .collect();
                        for (label, fexpr) in args {
                            let fty = self.check_expr(fexpr)?;
                            if let Some(fname) = label {
                                if let Some(expected_ty) = def_map.remove(fname) {
                                    if fty != expected_ty {
                                        self.reporter.report(
                                            Diagnostic::error(format!(
                                                "Field '{}' expects type {:?}, got {:?}",
                                                fname, expected_ty, fty
                                            ))
                                            .with_span(*span)
                                            .with_code(ErrorCode::TypeMismatch),
                                        );
                                        return Err(format!(
                                            "Field '{}' expects type {:?}, got {:?}",
                                            fname, expected_ty, fty
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
                                    Diagnostic::error(
                                        "Variant instantiation requires named arguments",
                                    )
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
                }

                for (_, arg) in args {
                    self.check_expr(arg)?;
                }
                if let Ty::Function(_, ret_ty) = callee_ty {
                    return Ok(*ret_ty);
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
                self.check_block(body, None)?;
                Ok(Ty::Int)
            }
            Expr::Assign {
                target,
                value,
                span,
            } => {
                // Determine target type without checking initialization for Idents
                let target_ty = match &**target {
                    Expr::Ident(id, _, _) => {
                        self.used_bindings.insert(*id);
                        self.env
                            .get(id)
                            .cloned()
                            .ok_or(format!("Cannot infer type for unbound variable"))?
                    }
                    _ => self.check_expr(target)?,
                };

                // Mutability check
                if let Expr::Ident(id, _, _) = &**target {
                    let was_initialized = self.initialized_bindings.contains(id);
                    self.initialized_bindings.insert(*id); // Mark as initialized upon assignment
                    if let Some(&is_mut) = self.mutability_env.get(id) {
                        if !is_mut && was_initialized {
                            self.reporter.report(Diagnostic::error("Cannot reassign immutable variable")
                                .with_span(*span)
                                .with_code(ErrorCode::ImmutableAssignment)
                                .with_hint("Declare this variable with 'var' instead of 'let' to make it mutable"));
                        }
                    }
                } else if let Expr::MemberAccess { object, member, .. } = &**target {
                    if let Expr::Ident(id, _, _) = &**object {
                        if let Some(&is_mut) = self.mutability_env.get(id) {
                            if !is_mut {
                                self.reporter.report(Diagnostic::error("Cannot mutate field of immutable variable")
                                    .with_span(*span)
                                    .with_code(ErrorCode::ImmutableAssignment)
                                    .with_hint("Declare this variable with 'var' instead of 'let' to make it mutable"));
                            }
                        }
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
                            for (fname, _, is_mut) in fields {
                                if fname == member {
                                    field_is_mut = *is_mut;
                                    break;
                                }
                            }
                        }
                    }

                    if !field_is_mut {
                        let mut allowed = false;
                        if let Expr::Ident(_, name, _) = &**object {
                            if name == "self" {
                                if let Some(fn_name) = &self.current_fn_name {
                                    if fn_name.ends_with("_init") || fn_name == "init" {
                                        allowed = true;
                                    }
                                }
                            }
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
                if target_ty != val_ty {
                    self.reporter.report(
                        Diagnostic::error(format!(
                            "Type mismatch in assignment: expected {:?}, got {:?}",
                            target_ty, val_ty
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
                span: _,
            } => {
                let subject_ty = self.check_expr(subject)?;
                let mut ret_ty = None;

                for arm in arms {
                    let mut arm_env = HashMap::new();
                    if let pace_hir::Pattern::Variant { name, fields, .. } = &arm.pattern {
                        if let Ty::Enum(enum_id) = subject_ty {
                            if let Some(variants) = self.enum_defs.get(&enum_id).cloned() {
                                if let Some(v) = variants.iter().find(|v| v.name == *name) {
                                    if let Some(vfields) = &v.fields {
                                        if let Some(pfields) = fields {
                                            if vfields.len() == pfields.len() {
                                                for (i, (pf_id, _, _)) in pfields.iter().enumerate()
                                                {
                                                    let fty = self
                                                        .resolve_type(&vfields[i].1)
                                                        .unwrap_or(Ty::Int);
                                                    arm_env.insert(*pf_id, fty.clone());
                                                    self.local_types.insert(*pf_id, fty);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    let outer_env = self.env.clone();
                    for (k, v) in arm_env {
                        self.env.insert(k, v);
                    }

                    let arm_ty = self.check_expr(&arm.body)?;
                    self.env = outer_env;

                    if ret_ty.is_none() {
                        ret_ty = Some(arm_ty);
                    } else if ret_ty != Some(arm_ty.clone()) {
                        self.reporter.report(
                            Diagnostic::error(format!(
                                "Match arms have incompatible types: {:?} and {:?}",
                                ret_ty.as_ref().unwrap(),
                                arm_ty
                            ))
                            .with_span(arm.span)
                            .with_code(ErrorCode::TypeMismatch),
                        );
                    }
                }

                Ok(ret_ty.unwrap_or(Ty::Void))
            }
        }
    }

    pub fn instantiate_generic(
        &mut self,
        template_name: &str,
        args: &[pace_ast::Type],
    ) -> Result<Ty, String> {
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
        for (i, (param_name, default_ty)) in generic_params.iter().enumerate() {
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
        for ((param_name, _), arg) in generic_params.into_iter().zip(final_args.iter()) {
            mapping.insert(param_name, arg.clone());
            let arg_ty = self.resolve_type(arg)?;
            mono_name.push_str(
                &format!("_{:?}", arg_ty)
                    .replace(" ", "")
                    .replace("(", "_")
                    .replace(")", "_")
                    .replace(":", "_"),
            );
        }

        if let Some(&(hir_id, kind)) = self.named_types.get(&mono_name) {
            if kind == 1 {
                return Ok(Ty::Class(hir_id));
            } else if kind == 0 {
                return Ok(Ty::Struct(hir_id));
            } else {
                return Ok(Ty::Enum(hir_id));
            }
        }

        let mut new_decl = template.clone();
        match &mut new_decl {
            pace_hir::Decl::Struct {
                id,
                name,
                fields,
                static_fields,
                const_fields,
                generic_params,
                ..
            }
            | pace_hir::Decl::Class {
                id,
                name,
                fields,
                static_fields,
                const_fields,
                generic_params,
                ..
            } => {
                *id = self.generate_id();
                *name = mono_name.clone();
                *generic_params = None;
                for (_, ty, _, _) in fields.iter_mut() {
                    *ty = substitute_type(ty, &mapping);
                }
                for (_, ty, _) in static_fields.iter_mut() {
                    *ty = substitute_type(ty, &mapping);
                }
                for (_, ty, _) in const_fields.iter_mut() {
                    *ty = substitute_type(ty, &mapping);
                }
            }
            pace_hir::Decl::Enum {
                id,
                name,
                variants,
                generic_params,
                ..
            } => {
                *id = self.generate_id();
                *name = mono_name.clone();
                *generic_params = None;
                for v in variants.iter_mut() {
                    v.id = self.generate_id();
                    if let Some(fields) = &mut v.fields {
                        for (_, ty) in fields.iter_mut() {
                            *ty = substitute_type(ty, &mapping);
                        }
                    }
                }
            }
            pace_hir::Decl::Function {
                id,
                name,
                params,
                return_type,
                generic_params,
                ..
            } => {
                *id = self.generate_id();
                *name = mono_name.clone();
                *generic_params = None;
                for (_, _, ty) in params.iter_mut() {
                    *ty = substitute_type(ty, &mapping);
                }
                if let Some(rty) = return_type {
                    *rty = substitute_type(rty, &mapping);
                }
                // We should theoretically substitute types in the body too, but since type resolution
                // happens on the fly in `check_expr` through `env` and `named_types`, the AST substitution
                // is mostly needed for declarations.
            }
            _ => {}
        }

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

        self.check_decl(&new_decl)?;

        match new_decl {
            pace_hir::Decl::Struct { id, fields, .. } => {
                let mut resolved_fields = Vec::new();
                for (fname, fty, _, is_mut) in fields {
                    resolved_fields.push((fname, self.resolve_type(&fty)?, is_mut));
                }
                self.struct_defs.insert(id, resolved_fields);
                self.named_types.insert(mono_name.clone(), (id, 0));
                Ok(Ty::Struct(id))
            }
            pace_hir::Decl::Class { id, fields, .. } => {
                let mut resolved_fields = Vec::new();
                for (fname, fty, _, is_mut) in fields {
                    resolved_fields.push((fname, self.resolve_type(&fty)?, is_mut));
                }
                self.class_defs.insert(id, resolved_fields);
                self.named_types.insert(mono_name.clone(), (id, 1));
                Ok(Ty::Class(id))
            }
            pace_hir::Decl::Enum { id, variants, .. } => {
                self.named_types.insert(mono_name.clone(), (id, 2));
                self.enum_defs.insert(id, variants.clone());

                for v in variants {
                    if let Some(f) = &v.fields {
                        let mut param_tys = Vec::new();
                        for (_, fty) in f {
                            param_tys.push(self.resolve_type(fty).unwrap_or(Ty::Int));
                        }
                        self.env
                            .insert(v.id, Ty::Function(param_tys, Box::new(Ty::Enum(id))));
                    } else {
                        self.env.insert(v.id, Ty::Enum(id));
                    }
                }
                Ok(Ty::Enum(id))
            }
            pace_hir::Decl::Function {
                params,
                return_type,
                ..
            } => {
                let mut param_tys = Vec::new();
                for (_, _, ty) in params {
                    param_tys.push(self.resolve_type(&ty)?);
                }
                let ret_ty = if let Some(rty) = return_type {
                    self.resolve_type(&rty)?
                } else {
                    Ty::Void
                };

                let func_ty = Ty::Function(param_tys, Box::new(ret_ty));
                Ok(func_ty)
            }
            _ => unreachable!(),
        }
    }
}

pub fn substitute_type(
    ty: &pace_ast::Type,
    mapping: &std::collections::HashMap<String, pace_ast::Type>,
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
    }
}
