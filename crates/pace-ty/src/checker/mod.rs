use crate::env::{Environment, Type};
use pace_errors::TypeError;
use std::collections::HashMap;

pub mod decl;
pub mod expr;
pub mod stmt;
pub mod cycle;

impl<'a> TypeChecker<'a> {
    pub fn get_span_for(&self, token: &str) -> pace_span::Span {
        if let Some(src) = self.sources.get(&self.current_module)
            && let Some(idx) = src.find(token)
        {
            return pace_span::Span::new(idx, idx + token.len());
        }
        pace_span::Span::default()
    }

    pub fn get_source(&self) -> miette::NamedSource<String> {
        let name = self.current_module;
        let src = self.sources.get(&name).cloned().unwrap_or_default();
        miette::NamedSource::new(name, src)
    }
}
pub struct TypeChecker<'a> {
    pub file_name: String,
    pub sources: HashMap<ustr::Ustr, String>,
    pub env: Environment,
    current_return_type: Option<Type>,
    current_class: Option<ustr::Ustr>,
    current_module: ustr::Ustr,
    generic_params_in_scope: Vec<pace_ast::GenericParam>,
    pub warnings: Vec<pace_errors::SemanticWarning>,
    pub errors: Vec<TypeError>,
    pub current_span: pace_span::Span,
    pub arena: &'a pace_hir::HirArena,
}

pub fn check(
    arena: &pace_hir::HirArena,
    ast: &[pace_hir::StmtId],
    sources: HashMap<ustr::Ustr, String>,
    entry_module: &str,
) -> (
    Vec<pace_errors::SemanticWarning>,
    Vec<TypeError>,
    Environment,
) {
    let mut checker = TypeChecker::new(arena, sources, entry_module);

    // We set the initial current_module to the entry module
    checker.current_module = entry_module.to_string().into();
    checker.check(ast);
    (checker.warnings, checker.errors, checker.env)
}

impl<'a> TypeChecker<'a> {
    pub fn new(
        arena: &'a pace_hir::HirArena,
        sources: HashMap<ustr::Ustr, String>,
        file_name: &str,
    ) -> Self {
        Self {
            file_name: file_name.to_string(),
            sources,
            env: Environment::new(),
            current_return_type: None,
            current_class: None,
            current_module: "main".into(),
            generic_params_in_scope: Vec::new(),
            warnings: Vec::new(),
            errors: Vec::new(),
            current_span: pace_span::Span::default(),
            arena,
        }
    }

    pub(crate) fn define_var(
        &mut self,
        name: ustr::Ustr,
        ty: Type,
        span: pace_span::Span,
        is_mutable: bool,
    ) {
        if let Err(original_span) = self.env.define(name, ty, span, is_mutable) {
            self.errors.push(TypeError::DuplicateDeclaration {
                name: name.to_string(),
                src: self.get_source(),
                span: span,
                original_span: original_span,
            });
        }
    }

    pub fn check(&mut self, stmts: &[pace_hir::StmtId]) {
        // Pass 1: Register all types
        self.register_types(stmts);

        // Pass 2: Resolve all signatures
        self.resolve_signatures(stmts);

        // Pass 3: Detect potential cycles
        self.detect_cycles();

        // Pass 4: Checking bodies
        for stmt_id in stmts {
            self.check_stmt(*stmt_id);
        }

        self.pop_scope_and_check_unused();
    }

    pub(crate) fn pop_scope_and_check_unused(&mut self) {
        let unused = self.env.pop_scope();
        for (name, var_info) in unused {
            if !var_info.is_used
                && !name.starts_with('_')
                && name != "self"
                && var_info.span != pace_span::Span::default()
            {
                let kind = if matches!(var_info.ty, Type::Function { .. }) {
                    "function"
                } else {
                    "variable"
                };
                self.warnings
                    .push(pace_errors::SemanticWarning::UnusedItem {
                        kind: kind.to_string(),
                        name: name.to_string(),
                        src: self.get_source(),
                        span: var_info.span,
                    });
            }
        }
    }

    pub(crate) fn resolve_type_name(&mut self, annotation: &pace_ast::TypeAnnotation) -> Type {
        let base_name = &annotation.name;

        let mut base_type = match base_name.as_str() {
            "Int" => Type::Int,
            "Float" => Type::Float,
            "String" => Type::String,
            "Bool" => Type::Bool,
            "Void" => Type::Void,
            "Self" => {
                if let Some(current) = &self.current_class {
                    if self.env.structs.contains_key(&ustr::Ustr::from(current)) {
                        Type::Struct(ustr::Ustr::from(current))
                    } else if self.env.enums.contains_key(&ustr::Ustr::from(current)) {
                        Type::Enum(ustr::Ustr::from(current))
                    } else if self.env.actors.contains_key(&ustr::Ustr::from(current)) {
                        Type::Actor(ustr::Ustr::from(current))
                    } else {
                        Type::Class(ustr::Ustr::from(current))
                    }
                } else {
                    Type::Unknown // Self used outside a class/struct/enum context
                }
            }
            _ => {
                let mut found_generic = None;
                for gp in &self.generic_params_in_scope {
                    if gp.name == ustr::Ustr::from(base_name) {
                        found_generic = Some(gp.clone());
                        break;
                    }
                }
                
                let ty = if let Some(gp) = found_generic {
                    let bound = if let Some(b) = &gp.bound { Some(Box::new(self.resolve_type_name(b))) } else { None };
                    Type::GenericParameter(ustr::Ustr::from(base_name), bound)
                } else if self.env.structs.contains_key(&ustr::Ustr::from(base_name)) {
                    Type::Struct(ustr::Ustr::from(base_name))
                } else if self.env.enums.contains_key(&ustr::Ustr::from(base_name)) {
                    Type::Enum(ustr::Ustr::from(base_name))
                } else if self.env.actors.contains_key(&ustr::Ustr::from(base_name)) {
                    Type::Actor(ustr::Ustr::from(base_name))
                } else if self.env.interfaces.contains_key(&ustr::Ustr::from(base_name)) {
                    Type::Interface(ustr::Ustr::from(base_name))
                } else {
                    Type::Class(ustr::Ustr::from(base_name))
                };
                base_name.starts_with("Result_");
                ty
            }
        };

        if !annotation.args.is_empty() {
            let mut arg_types = Vec::new();
            for arg in &annotation.args {
                arg_types.push(self.resolve_type_name(arg));
            }
            
            let expected_params = match &base_type {
                Type::Class(name) => self.env.classes.get(name).and_then(|s| s.generic_params.clone()),
                Type::Struct(name) => self.env.structs.get(name).and_then(|s| s.generic_params.clone()),
                Type::Enum(name) => self.env.enums.get(name).and_then(|s| s.generic_params.clone()),
                Type::Interface(name) => self.env.interfaces.get(name).and_then(|s| s.generic_params.clone()),
                Type::Actor(name) => self.env.actors.get(name).and_then(|s| s.generic_params.clone()),
                _ => None,
            };

            if let Some(params) = expected_params {
                if params.len() == arg_types.len() {
                    for (param, arg_ty) in params.iter().zip(arg_types.iter()) {
                        if let Some(bound_annotation) = &param.bound {
                            let bound_ty = self.resolve_type_name(bound_annotation);
                            if !self.is_assignable_to(arg_ty, &bound_ty) {
                                self.errors.push(pace_errors::TypeError::Generic {
                                    src: self.get_source(),
                                    span: self.current_span, // we might not have a perfect span for the generic arg, so current_span is used
                                    message: format!("Type '{:?}' does not satisfy bound '{:?}' for generic parameter '{}'", arg_ty, bound_ty, param.name),
                                });
                            }
                        }
                    }
                }
            }

            base_type = Type::GenericInstance {
                base: Box::new(base_type),
                args: arg_types,
            };
        }

        if annotation.is_function {
            let mut params = Vec::new();
            if let Some(fn_params) = &annotation.function_params {
                for p in fn_params {
                    params.push(self.resolve_type_name(p));
                }
            }
            let return_type = if let Some(ret) = &annotation.function_return {
                Box::new(self.resolve_type_name(ret))
            } else {
                Box::new(Type::Void)
            };
            base_type = Type::Function {
                generic_params: None,
                params,
                return_type,
            };
        }

        if annotation.is_nullable {
            Type::Nullable(Box::new(base_type))
        } else {
            base_type
        }
    }
    pub fn is_assignable_to(&self, source: &Type, target: &Type) -> bool {
        if source == target {
            return true;
        }
        if matches!(source, Type::Unknown | Type::Any) || matches!(target, Type::Unknown | Type::Any) {
            return true;
        }

        // Generic parameter bound subtyping
        if let Type::GenericParameter(_, Some(bound)) = source {
            return self.is_assignable_to(bound, target);
        }
        
        let to_concrete = |ty: &Type| -> Option<Type> {
            if let Type::GenericInstance { base, args } = ty {
                if let Type::Class(name) = &**base {
                    let mut concrete_name = name.as_str().to_string();
                    for arg in args {
                        let arg_name = format!("{:?}", arg);
                        concrete_name.push('_');
                        concrete_name.push_str(&arg_name.replace(" ", "_"));
                    }
                    return Some(Type::Class(ustr::Ustr::from(concrete_name.as_str())));
                } else if let Type::Interface(name) = &**base {
                    let mut concrete_name = name.as_str().to_string();
                    for arg in args {
                        let arg_name = format!("{:?}", arg);
                        concrete_name.push('_');
                        concrete_name.push_str(&arg_name.replace(" ", "_"));
                    }
                    return Some(Type::Interface(ustr::Ustr::from(concrete_name.as_str())));
                }
            }
            None
        };
        
        let src_eff = to_concrete(source).unwrap_or_else(|| source.clone());
        let tgt_eff = to_concrete(target).unwrap_or_else(|| target.clone());
        
        if src_eff == tgt_eff {
            return true;
        }
        
        // Subtyping: Class/Actor implements Interface
        if let Type::Interface(iface_name) = &tgt_eff {
            if let Type::Class(class_name) = &src_eff {
                if let Some(class_sig) = self.env.classes.get(class_name) {
                    let impl_eff = if let Some(impl_ty) = &class_sig.implements {
                        to_concrete(impl_ty).unwrap_or_else(|| impl_ty.clone())
                    } else {
                        Type::Unknown
                    };
                    if let Type::Interface(impl_name) = impl_eff {
                        if &impl_name == iface_name {
                            return true;
                        }
                    }
                }
            } else if let Type::Actor(actor_name) = &src_eff {
                if let Some(actor_sig) = self.env.actors.get(actor_name) {
                    let impl_eff = if let Some(impl_ty) = &actor_sig.implements {
                        to_concrete(impl_ty).unwrap_or_else(|| impl_ty.clone())
                    } else {
                        Type::Unknown
                    };
                    if let Type::Interface(impl_name) = impl_eff {
                        if &impl_name == iface_name {
                            return true;
                        }
                    }
                }
            }
        }
        
        // List -> Set coercion (mainly for Array Literals)
        if let Type::Class(src_name) = &src_eff {
            if let Type::Class(tgt_name) = &tgt_eff {
                let src_str = src_name.as_str();
                let tgt_str = tgt_name.as_str();
                
                if (src_str.starts_with("pace_collections_list__List_") || src_str.starts_with("List_")) &&
                   (tgt_str.starts_with("pace_collections_set__Set_") || tgt_str.starts_with("Set_")) {
                    
                    let src_suffix = if src_str.starts_with("pace_collections_list__List_") {
                        &src_str["pace_collections_list__List_".len()..]
                    } else {
                        &src_str["List_".len()..]
                    };
                    
                    let tgt_suffix = if tgt_str.starts_with("pace_collections_set__Set_") {
                        &tgt_str["pace_collections_set__Set_".len()..]
                    } else {
                        &tgt_str["Set_".len()..]
                    };
                    
                    if src_suffix == tgt_suffix {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn check_pattern(&mut self, pattern: &pace_hir::Pattern, expected_type: &Type) {
        match pattern {
            pace_hir::Pattern::Wildcard => (),
            pace_hir::Pattern::Literal(expr) => {
                let ty = self.check_expr(*expr);
                if !self.is_assignable_to(&ty, expected_type) {
                    {
                        self.errors.push(TypeError::Generic {
                            src: self.get_source(),
                            span: self.current_span,
                            message: format!(
                                "Pattern type mismatch: expected {:?}, got {:?}",
                                expected_type, ty
                            ),
                        });
                    }
                }
            }
            pace_hir::Pattern::Variable(name) => {
                self.define_var(*name, expected_type.clone(), self.current_span, false);
            }
            pace_hir::Pattern::Variant {
                enum_name,
                variant_name,
                fields,
                generic_args: _,
            } => {
                let mut field_types = vec![Type::Unknown; fields.as_ref().map_or(0, |f| f.len())];
                let resolved_enum_name = enum_name.or_else(|| {
                    if let Type::Enum(name) = expected_type {
                        Some(*name)
                    } else {
                        None
                    }
                });

                if let Some(ename) = resolved_enum_name
                    && let Some(sig) = self.env.enums.get(&ename)
                    && let Some((_, v_fields_opt)) = sig
                        .variants
                        .iter()
                        .find(|(name, _)| **name == *variant_name)
                    && let Some(v_fields) = v_fields_opt
                {
                    for (i, f_ty) in v_fields.iter().enumerate() {
                        if i < field_types.len() {
                            field_types[i] = f_ty.clone();
                        }
                    }
                }
                if let Some(fs) = fields {
                    for (i, pat) in fs.iter().enumerate() {
                        self.check_pattern(pat, &field_types[i]);
                    }
                }
            }
        }
    }
}

pub(crate) fn is_camel_case(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    let check_str = s.strip_prefix('_').unwrap_or(s);
    if check_str.is_empty() {
        return true;
    }
    let first = check_str.chars().next().unwrap();
    if first.is_uppercase() {
        return false;
    }
    !check_str.contains('_')
}
