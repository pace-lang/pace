use crate::env::{Type, Environment};
use pace_errors::TypeError;
use std::collections::HashMap;

pub mod expr;
pub mod stmt;
pub mod decl;
impl<'a> TypeChecker<'a> {
    pub fn get_span_for(&self, token: &str) -> pace_ast::Span {
        if let Some(src) = self.sources.get(&self.current_module)
            && let Some(idx) = src.find(token) {
                return pace_ast::Span::new(idx, token.len());
            }
        pace_ast::Span::default()
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
    generic_params_in_scope: Vec<ustr::Ustr>,
    pub warnings: Vec<pace_errors::SemanticWarning>,
    pub errors: Vec<TypeError>,
    pub current_span: pace_ast::Span,
    pub arena: &'a pace_ast::arena::AstArena,
}

pub fn check(
    arena: &pace_ast::arena::AstArena,
    ast: &[pace_ast::arena::StmtId],
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
    pub fn new(arena: &'a pace_ast::arena::AstArena, sources: HashMap<ustr::Ustr, String>, file_name: &str) -> Self {
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
            current_span: pace_ast::Span::default(),
            arena,
        }
    }

    pub(crate) fn define_var(&mut self, name: ustr::Ustr, ty: Type, span: pace_ast::Span, is_mutable: bool) {
        if let Err(original_span) = self.env.define(name, ty, span, is_mutable) {
            self.errors.push(TypeError::DuplicateDeclaration {
                name: name.to_string(),
                src: self.get_source(),
                span: span.into(),
                original_span: original_span.into(),
            });
        }
    }

    pub fn check(&mut self, stmts: &[pace_ast::arena::StmtId]) {
        // Pass 1: Register all types
        self.register_types(stmts);

        // Pass 2: Resolve all signatures
        self.resolve_signatures(stmts);

        // Pass 2: Checking bodies
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
                && var_info.span != pace_ast::Span::default()
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
                        span: var_info.span.into(),
                    });
            }
        }
    }

    pub(crate) fn resolve_type_name(&self, annotation: &pace_ast::TypeAnnotation) -> Type {
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
                let ty = if self.generic_params_in_scope.contains(&ustr::Ustr::from(base_name)) {
                    Type::GenericParameter(ustr::Ustr::from(base_name))
                } else if self.env.structs.contains_key(&ustr::Ustr::from(base_name)) {
                    Type::Struct(ustr::Ustr::from(base_name))
                } else if self.env.enums.contains_key(&ustr::Ustr::from(base_name)) {
                    Type::Enum(ustr::Ustr::from(base_name))
                } else if self.env.actors.contains_key(&ustr::Ustr::from(base_name)) {
                    Type::Actor(ustr::Ustr::from(base_name))
                } else {
                    Type::Class(ustr::Ustr::from(base_name))
                };
                if base_name.starts_with("Result_") {
                }
                ty
            }
        };

        if !annotation.args.is_empty() {
            let mut arg_types = Vec::new();
            for arg in &annotation.args {
                arg_types.push(self.resolve_type_name(arg));
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
    pub fn check_pattern(&mut self, pattern: &pace_ast::Pattern, expected_type: &Type) {
        match pattern {
            pace_ast::Pattern::Wildcard => (),
            pace_ast::Pattern::Literal(expr) => {
                let ty = self.check_expr(*expr);
                if expected_type != &ty && expected_type != &Type::Unknown && ty != Type::Unknown {
                    {
                        self.errors.push(TypeError::Generic {
                            src: self.get_source(),
                            span: self.current_span.into(),
                            message: format!(
                                "Pattern type mismatch: expected {:?}, got {:?}",
                                expected_type, ty
                            ),
                        });
                    }
                }
                
            }
            pace_ast::Pattern::Variable(name, span) => {
                self.define_var(*name, expected_type.clone(), *span, false);
                
            }
            pace_ast::Pattern::Variant {
                enum_name,
                variant_name,
                fields,
                generic_args: _,
            } => {
                let mut field_types = vec![Type::Unknown; fields.as_ref().map_or(0, |f| f.len())];
                if let Some(ename) = enum_name
                    && let Some(sig) = self.env.enums.get(&ustr::Ustr::from(ename))
                        && let Some((_, v_fields_opt)) = sig
                            .variants
                            .iter()
                            .find(|(name, _)| **name == *variant_name)
                            && let Some(v_fields) = v_fields_opt {
                                for (i, f_ty) in v_fields.iter().enumerate() {
                                    if i < field_types.len() {
                                        field_types[i] = f_ty.clone();
                                    }
                                }
                            }

                if let Some(fields) = fields {
                    for (i, field) in fields.iter().enumerate() {
                        self.check_pattern(field, &field_types[i]);
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
