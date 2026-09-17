use crate::hir::*;
use pace_ast as ast;

use super::LoweringContext;

impl LoweringContext {
    pub(crate) fn lower_decl(&mut self, decl: ast::Decl) -> Result<Option<Decl>, String> {
        match decl {
            ast::Decl::Import { path, alias, span } => Ok(Some(Decl::Import {
                path: path.into_iter().map(|i| i.name).collect(),
                alias: alias.map(|i| i.name),
                span,
            })),
            ast::Decl::Let {
                name,
                ty,
                value,
                is_private,
                span,
            } => {
                let id = self.generate_id();
                self.scope.insert(name.name.clone(), id);
                let lowered_value = match value {
                    Some(expr) => Some(self.lower_expr(expr)?),
                    None => None,
                };
                Ok(Some(Decl::Let {
                    id,
                    name: name.name,
                    ty,
                    value: lowered_value,
                    is_private,
                    span,
                }))
            }
            ast::Decl::Var {
                name,
                ty,
                value,
                is_private,
                span,
            } => {
                let id = self.generate_id();
                self.scope.insert(name.name.clone(), id);
                let lowered_value = match value {
                    Some(expr) => Some(self.lower_expr(expr)?),
                    None => None,
                };
                Ok(Some(Decl::Var {
                    id,
                    name: name.name,
                    ty,
                    value: lowered_value,
                    is_private,
                    span,
                }))
            }
            ast::Decl::Struct {
                name,
                generic_params,
                with,
                fields,
                static_fields,
                const_fields,
                methods,
                is_private,
                span,
                ..
            } => {
                let id = self.generate_id();
                self.scope.insert(name.name.clone(), id);
                let mut lowered_fields = Vec::new();
                for (field_name, field_ty, field_val, is_mut, is_field_private) in fields.clone() {
                    let lowered_val = match field_val {
                        Some(v) => Some(self.lower_expr(v)?),
                        None => None,
                    };
                    lowered_fields.push((
                        field_name.name.clone(),
                        field_ty.clone(),
                        lowered_val,
                        is_mut,
                        is_field_private,
                    ));
                }

                let mut lowered_static = Vec::new();
                for (sf_name, sf_ty, sf_val, is_field_private) in static_fields {
                    lowered_static.push((sf_name.name, sf_ty, self.lower_expr(sf_val)?, is_field_private));
                }
                let mut lowered_const = Vec::new();
                for (cf_name, cf_ty, cf_val, is_field_private) in const_fields {
                    lowered_const.push((cf_name.name, cf_ty, self.lower_expr(cf_val)?, is_field_private));
                }

                let mut initializers = Vec::new();
                for (field_name, _, field_val, _, _) in &fields {
                    if let Some(val) = field_val {
                        let lhs = ast::Expr::MemberAccess {
                            object: Box::new(ast::Expr::Ident(
                                ast::Ident {
                                    name: "self".to_string(),
                                    span: field_name.span,
                                },
                                None,
                            )),
                            member: field_name.clone(),
                            span: field_name.span,
                        };
                        let stmt = ast::Stmt::ExprStmt(
                            ast::Expr::Assign {
                                target: Box::new(lhs),
                                value: Box::new(val.clone()),
                                span: val.span(),
                            },
                            val.span(),
                        );
                        initializers.push(stmt);
                    }
                }

                let mut methods = methods;
                if !initializers.is_empty() {
                    let mut has_init = false;
                    for method in &mut methods {
                        if let ast::Decl::Function {
                            name: m_name, body, ..
                        } = method
                        {
                            if m_name.name == "init" {
                                has_init = true;
                                for (i, init) in initializers.iter().enumerate() {
                                    body.statements.insert(i, init.clone());
                                }
                            }
                        }
                    }
                    if !has_init {
                        let synthetic_init = ast::Decl::Function {
                            name: ast::Ident {
                                name: "init".to_string(),
                                span,
                            },
                            generic_params: None,
                            params: vec![],
                            return_type: None,
                            body: ast::Block {
                                statements: initializers,
                                span,
                            },
                            is_static: false,
                            is_override: false,
                            is_private: false,
                            span,
                        };
                        methods.push(synthetic_init);
                    }
                }

                let mut lowered_methods = Vec::new();
                for method in methods {
                    if let ast::Decl::Function {
                        name: m_name,
                        generic_params: m_generic_params,
                        params,
                        return_type,
                        body,
                        span: m_span,
                        is_static,
                        is_override,
                        is_private: is_method_private,
                    } = method
                    {
                        let mut new_params = Vec::new();
                        if !is_static {
                            new_params.push((
                                ast::Ident {
                                    name: "self".to_string(),
                                    span: m_name.span,
                                },
                                ast::Type::Named(name.clone()),
                            ));
                        }
                        new_params.extend(params);
                        let m_decl = ast::Decl::Function {
                            name: ast::Ident {
                                name: format!("{}_{}", name.name, m_name.name),
                                span: m_name.span,
                            },
                            generic_params: m_generic_params,
                            params: new_params,
                            return_type,
                            body,
                            span: m_span,
                            is_static,
                            is_override,
                            is_private: is_method_private,
                        };
                        if let Some(lowered) = self.lower_decl(m_decl)? {
                            lowered_methods.push(lowered);
                        }
                    }
                }
                let hir_generic_params = generic_params.map(|params| {
                    params
                        .into_iter()
                        .map(|p| (p.name.name, p.default))
                        .collect()
                });
                let hir_with = with.into_iter().map(|w| w.name).collect();
                Ok(Some(Decl::Struct {
                    id,
                    name: name.name,
                    generic_params: hir_generic_params,
                    with: hir_with,
                    fields: lowered_fields,
                    static_fields: lowered_static,
                    const_fields: lowered_const,
                    methods: lowered_methods,
                    is_private,
                    span,
                }))
            }
            ast::Decl::Class {
                name,
                generic_params,
                extends,
                with,
                fields,
                static_fields,
                const_fields,
                methods,
                is_private,
                span,
                ..
            } => {
                let id = self.generate_id();
                self.scope.insert(name.name.clone(), id);
                let mut lowered_fields = Vec::new();
                for (field_name, field_ty, field_val, is_mut, is_field_private) in fields.clone() {
                    let lowered_val = match field_val {
                        Some(v) => Some(self.lower_expr(v)?),
                        None => None,
                    };
                    lowered_fields.push((field_name.name, field_ty, lowered_val, is_mut, is_field_private));
                }

                let mut lowered_static = Vec::new();
                for (sf_name, sf_ty, sf_val, is_field_private) in static_fields {
                    lowered_static.push((sf_name.name, sf_ty, self.lower_expr(sf_val)?, is_field_private));
                }
                let mut lowered_const = Vec::new();
                for (cf_name, cf_ty, cf_val, is_field_private) in const_fields {
                    lowered_const.push((cf_name.name, cf_ty, self.lower_expr(cf_val)?, is_field_private));
                }

                let mut initializers = Vec::new();
                for (field_name, _, field_val, _, _) in &fields {
                    if let Some(val) = field_val {
                        let lhs = ast::Expr::MemberAccess {
                            object: Box::new(ast::Expr::Ident(
                                ast::Ident {
                                    name: "self".to_string(),
                                    span: field_name.span,
                                },
                                None,
                            )),
                            member: field_name.clone(),
                            span: field_name.span,
                        };
                        let stmt = ast::Stmt::ExprStmt(
                            ast::Expr::Assign {
                                target: Box::new(lhs),
                                value: Box::new(val.clone()),
                                span: val.span(),
                            },
                            val.span(),
                        );
                        initializers.push(stmt);
                    }
                }

                let mut methods = methods;
                if !initializers.is_empty() {
                    let mut has_init = false;
                    for method in &mut methods {
                        if let ast::Decl::Function {
                            name: m_name, body, ..
                        } = method
                        {
                            if m_name.name == "init" {
                                has_init = true;
                                let mut inject_idx = 0;
                                if let Some(first_stmt) = body.statements.first() {
                                    if let ast::Stmt::ExprStmt(ast::Expr::Call { callee, .. }, _) =
                                        first_stmt
                                    {
                                        if let ast::Expr::MemberAccess { object, member, .. } =
                                            &**callee
                                        {
                                            if let ast::Expr::Super(_) = &**object {
                                                if member.name == "init" {
                                                    inject_idx = 1;
                                                }
                                            }
                                        }
                                    }
                                }
                                for (i, init) in initializers.iter().enumerate() {
                                    body.statements.insert(inject_idx + i, init.clone());
                                }
                            }
                        }
                    }
                    if !has_init {
                        let synthetic_init = ast::Decl::Function {
                            name: ast::Ident {
                                name: "init".to_string(),
                                span,
                            },
                            generic_params: None,
                            params: vec![],
                            return_type: None,
                            body: ast::Block {
                                statements: initializers,
                                span,
                            },
                            is_static: false,
                            is_override: false,
                            is_private: false,
                            span,
                        };
                        methods.push(synthetic_init);
                    }
                }

                let mut lowered_methods = Vec::new();
                for method in methods {
                    if let ast::Decl::Function {
                        name: m_name,
                        generic_params: m_generic_params,
                        params,
                        return_type,
                        body,
                        span: m_span,
                        is_static,
                        is_override,
                        is_private: is_method_private,
                    } = method
                    {
                        let mut new_params = Vec::new();
                        if !is_static {
                            new_params.push((
                                ast::Ident {
                                    name: "self".to_string(),
                                    span: m_name.span,
                                },
                                ast::Type::Named(name.clone()),
                            ));
                        }
                        new_params.extend(params);
                        let m_decl = ast::Decl::Function {
                            name: ast::Ident {
                                name: format!("{}_{}", name.name, m_name.name),
                                span: m_name.span,
                            },
                            generic_params: m_generic_params,
                            params: new_params,
                            return_type,
                            body,
                            span: m_span,
                            is_static,
                            is_override,
                            is_private: is_method_private,
                        };
                        if let Some(lowered) = self.lower_decl(m_decl)? {
                            lowered_methods.push(lowered);
                        }
                    }
                }
                let hir_generic_params = generic_params.map(|params| {
                    params
                        .into_iter()
                        .map(|p| (p.name.name, p.default))
                        .collect()
                });
                let hir_with = with.into_iter().map(|w| w.name).collect();
                let hir_extends = extends.map(|e| e.name);
                Ok(Some(Decl::Class {
                    id,
                    name: name.name,
                    generic_params: hir_generic_params,
                    extends: hir_extends,
                    with: hir_with,
                    fields: lowered_fields,
                    static_fields: lowered_static,
                    const_fields: lowered_const,
                    methods: lowered_methods,
                    is_private,
                    span,
                }))
            }
            ast::Decl::Trait {
                name,
                generic_params,
                methods,
                is_private,
                span,
                ..
            } => {
                let id = self.generate_id();
                self.scope.insert(name.name.clone(), id);

                let mut lowered_methods = Vec::new();
                for method in methods {
                    if let ast::Decl::Function {
                        name: m_name,
                        generic_params: m_generic_params,
                        params,
                        return_type,
                        body,
                        span: m_span,
                        is_static,
                        is_override,
                        is_private: is_method_private,
                    } = method
                    {
                        let mut new_params = Vec::new();
                        if !is_static {
                            new_params.push((
                                ast::Ident {
                                    name: "self".to_string(),
                                    span: m_name.span,
                                },
                                ast::Type::Named(name.clone()),
                            ));
                        }
                        new_params.extend(params);
                        let m_decl = ast::Decl::Function {
                            name: ast::Ident {
                                name: format!("{}_{}", name.name, m_name.name),
                                span: m_name.span,
                            },
                            generic_params: m_generic_params,
                            params: new_params,
                            return_type,
                            body,
                            span: m_span,
                            is_static,
                            is_override,
                            is_private: is_method_private,
                        };
                        if let Some(lowered) = self.lower_decl(m_decl)? {
                            lowered_methods.push(lowered);
                        }
                    }
                }
                let hir_generic_params = generic_params.map(|params| {
                    params
                        .into_iter()
                        .map(|p| (p.name.name, p.default))
                        .collect()
                });
                Ok(Some(Decl::Trait {
                    id,
                    name: name.name,
                    generic_params: hir_generic_params,
                    methods: lowered_methods,
                    is_private,
                    span,
                }))
            }
            ast::Decl::Expr(expr, span) => {
                let lowered = self.lower_expr(expr)?;
                Ok(Some(Decl::Expr(lowered, span)))
            }
            ast::Decl::Enum {
                name,
                generic_params,
                variants,
                is_private,
                span,
            } => {
                let id = self.generate_id();
                self.scope.insert(name.name.clone(), id);
                let mut lowered_variants = Vec::new();
                for v in variants {
                    let var_id = self.generate_id();
                    self.scope.insert(v.name.name.clone(), var_id);

                    let mut lowered_fields = None;
                    if let Some(fields) = v.fields {
                        let mut lf = Vec::new();
                        for (fname, fty) in fields {
                            lf.push((fname.name, fty));
                        }
                        lowered_fields = Some(lf);
                    }
                    lowered_variants.push(crate::hir::EnumVariant {
                        name: v.name.name,
                        id: var_id,
                        fields: lowered_fields,
                        span: v.span,
                    });
                }
                let hir_generic_params = generic_params.map(|params| {
                    params
                        .into_iter()
                        .map(|p| (p.name.name, p.default))
                        .collect()
                });
                Ok(Some(Decl::Enum {
                    id,
                    name: name.name,
                    generic_params: hir_generic_params,
                    variants: lowered_variants,
                    is_private,
                    span,
                }))
            }
            ast::Decl::Function {
                name,
                generic_params,
                params,
                return_type,
                body,
                span,
                is_static,
                is_override,
                is_private,
            } => {
                let id = self.generate_id();
                // We do NOT insert global functions into `self.scope` because they are resolved
                // by the TypeChecker using names and module imports.
                // However, if this is a method, we might need it? No, methods are resolved by TypeChecker too.
                // Wait! Let's just NOT insert it into the global scope.

                let outer_scope = self.scope.clone();
                let mut lowered_params = Vec::new();
                for (param_name, param_ty) in params {
                    let param_id = self.generate_id();
                    self.scope.insert(param_name.name.clone(), param_id);
                    lowered_params.push((param_id, param_name.name, param_ty));
                }

                let lowered_body = self.lower_block(body)?;
                self.scope = outer_scope;

                let hir_generic_params = generic_params.map(|params| {
                    params
                        .into_iter()
                        .map(|p| (p.name.name, p.default))
                        .collect()
                });
                Ok(Some(Decl::Function {
                    id,
                    name: name.name,
                    generic_params: hir_generic_params,
                    params: lowered_params,
                    return_type,
                    body: lowered_body,
                    is_static,
                    is_override,
                    is_private,
                    span,
                }))
            }
            _ => Ok(None),
        }
    }
}
