use pace_ast::{Expr, Stmt, TypeAnnotation};
use std::collections::HashMap;

pub struct Monomorphizer {
    pub generic_classes: HashMap<ustr::Ustr, pace_ast::arena::StmtId>,
    pub generic_class_modules: HashMap<ustr::Ustr, String>,
    pub generated_classes: HashMap<ustr::Ustr, pace_ast::arena::StmtId>,
    pub generic_funcs: HashMap<ustr::Ustr, pace_ast::arena::StmtId>,
    pub generic_func_modules: HashMap<ustr::Ustr, String>,
    pub generated_funcs: HashMap<ustr::Ustr, pace_ast::arena::StmtId>,
    pub all_interfaces: HashMap<ustr::Ustr, pace_ast::arena::StmtId>,
}

impl Default for Monomorphizer {
    fn default() -> Self {
        Self::new()
    }
}

impl Monomorphizer {
    pub fn new() -> Self {
        Self {
            generic_classes: HashMap::new(),
            generic_class_modules: HashMap::new(),
            generated_classes: HashMap::new(),
            generic_funcs: HashMap::new(),
            generic_func_modules: HashMap::new(),
            generated_funcs: HashMap::new(),
            all_interfaces: HashMap::new(),
        }
    }

    pub fn run(
        arena: &mut pace_ast::arena::AstArena,
        ast: Vec<pace_ast::arena::StmtId>,
    ) -> Result<Vec<pace_ast::arena::StmtId>, miette::Report> {
        let mut mono = Self::new();

        // Pass 1: Collect generic classes/actors and interfaces
        let mut final_ast = Vec::new();
        for &stmt_id in &ast {
            let stmt = arena.get_stmt(stmt_id).clone();
            if let Stmt::Module { name, body } = stmt {
                let mut new_body = Vec::new();
                for &inner_stmt_id in &body {
                    let inner_stmt = arena.get_stmt(inner_stmt_id).clone();
                    match inner_stmt {
                        Stmt::ClassDecl {
                            generic_params: Some(_),
                            name: class_name,
                            ..
                        }
                        | Stmt::ActorDecl {
                            generic_params: Some(_),
                            name: class_name,
                            ..
                        }
                        | Stmt::StructDecl {
                            generic_params: Some(_),
                            name: class_name,
                            ..
                        }
                        | Stmt::EnumDecl {
                            generic_params: Some(_),
                            name: class_name,
                            ..
                        }
                        | Stmt::InterfaceDecl {
                            generic_params: Some(_),
                            name: class_name,
                            ..
                        } => {
                            mono.generic_classes.insert(class_name, inner_stmt_id);
                            mono.generic_class_modules
                                .insert(class_name, name.as_str().to_string());
                        }
                        Stmt::FuncDecl {
                            generic_params: Some(_),
                            name: func_name,
                            ..
                        } => {
                            mono.generic_funcs.insert(func_name, inner_stmt_id);
                            mono.generic_func_modules
                                .insert(func_name, name.as_str().to_string());
                        }
                        Stmt::InterfaceDecl {
                            name: iface_name, ..
                        } => {
                            mono.all_interfaces.insert(iface_name, inner_stmt_id);
                            new_body.push(inner_stmt_id);
                        }
                        _ => {
                            new_body.push(inner_stmt_id);
                        }
                    }
                }

                let mod_stmt = Stmt::Module {
                    name,
                    body: new_body,
                };
                let mod_id = arena.alloc_stmt(mod_stmt, pace_ast::Span::default());
                final_ast.push(mod_id);
            } else {
                final_ast.push(stmt_id);
            }
        }

        // Pass 2: Rewrite AST and instantiate generics
        for &stmt_id in &final_ast {
            mono.rewrite_stmt(arena, stmt_id)?;
        }

        // Append all newly generated classes to the end of the AST
        let mut generated = Vec::new();
        for (concrete_name, instantiated_stmt_id) in mono.generated_classes {
            // Find original generic name (strip _TypeArgs...)
            let base_name = concrete_name
                .as_str()
                .split('_')
                .next()
                .unwrap_or(concrete_name.as_str())
                .to_string();
            let original_module = mono
                .generic_class_modules
                .get(&ustr::Ustr::from(base_name.as_str()))
                .cloned()
                .unwrap_or_else(|| "unknown_module".to_string());

            let mod_stmt = Stmt::Module {
                name: original_module.into(),
                body: vec![instantiated_stmt_id],
            };
            let mod_id = arena.alloc_stmt(mod_stmt, pace_ast::Span::default());
            generated.push(mod_id);
        }

        if let Some(&last_id) = final_ast.last() {
            let mut last_stmt = arena.get_stmt(last_id).clone();
            if let Stmt::Module { ref mut body, .. } = last_stmt {
                body.append(&mut generated);
                *arena.get_stmt_mut(last_id) = last_stmt;
            } else {
                final_ast.append(&mut generated);
            }
        } else {
            final_ast.append(&mut generated);
        }

        Ok(final_ast)
    }

    fn generate_name(base: &str, args: &[TypeAnnotation]) -> String {
        let mut name = base.to_string();
        for arg in args {
            name.push('_');
            name.push_str(&Self::generate_name(&arg.name, &arg.args));
        }
        name
    }

    fn rewrite_type_annotation(
        &mut self,
        arena: &mut pace_ast::arena::AstArena,
        ty: &mut pace_ast::TypeAnnotation,
    ) -> Result<(), miette::Report> {
        if !ty.args.is_empty() {
            for arg in &mut ty.args {
                self.rewrite_type_annotation(arena, arg)?;
            }

            let concrete_name = Self::generate_name(&ty.name, &ty.args);

            if !self
                .generated_classes
                .contains_key(&ustr::Ustr::from(&concrete_name))
            {
                if let Some(generic_decl) = self.generic_classes.get(&ty.name).cloned() {
                    self.instantiate_class(arena, generic_decl, concrete_name.clone(), &ty.args)?;
                } else {
                    // Fallback for prelude/core types that were not explicitly imported and not mangled by SymbolResolver
                    let mut found = None;
                    let target_suffix = format!("_{}", ty.name.as_str());
                    let target_suffix_2 = format!("__{}", ty.name.as_str());
                    for (k, v) in &self.generic_classes {
                        let k_str = k.as_str();
                        if k_str.ends_with(&target_suffix)
                            || k_str.ends_with(&target_suffix_2)
                            || k_str == ty.name.as_str()
                        {
                            found = Some(*v);
                            break;
                        }
                    }
                    if let Some(generic_decl) = found {
                        self.instantiate_class(
                            arena,
                            generic_decl,
                            concrete_name.clone(),
                            &ty.args,
                        )?;
                    }
                }
            }

            ty.name = concrete_name.into();
            ty.args.clear();
        }
        Ok(())
    }

    fn clone_stmt(
        &mut self,
        arena: &mut pace_ast::arena::AstArena,
        stmt_id: pace_ast::arena::StmtId,
    ) -> pace_ast::arena::StmtId {
        let stmt = arena.get_stmt(stmt_id).clone();
        let new_stmt = match stmt {
            Stmt::Module { name, body } => {
                let new_body = body
                    .into_iter()
                    .map(|s| self.clone_stmt(arena, s))
                    .collect();
                Stmt::Module {
                    name,
                    body: new_body,
                }
            }
            Stmt::FuncDecl {
                name,
                generic_params,
                params,
                return_type,
                body,
                is_async,
                is_static,
                is_extern,
                visibility,
                span,
            } => {
                let new_body = body
                    .into_iter()
                    .map(|s| self.clone_stmt(arena, s))
                    .collect();
                Stmt::FuncDecl {
                    name,
                    generic_params,
                    params,
                    return_type,
                    body: new_body,
                    is_async,
                    is_static,
                    is_extern,
                    visibility,
                    span,
                }
            }
            Stmt::ClassDecl {
                name,
                generic_params,
                fields,
                methods,
                implements,
            } => {
                let new_fields = fields
                    .into_iter()
                    .map(|f| self.clone_stmt(arena, f))
                    .collect();
                let new_methods = methods
                    .into_iter()
                    .map(|m| self.clone_stmt(arena, m))
                    .collect();
                Stmt::ClassDecl {
                    name,
                    generic_params,
                    fields: new_fields,
                    methods: new_methods,
                    implements,
                }
            }
            Stmt::ActorDecl {
                name,
                generic_params,
                fields,
                methods,
                implements,
            } => {
                let new_fields = fields
                    .into_iter()
                    .map(|f| self.clone_stmt(arena, f))
                    .collect();
                let new_methods = methods
                    .into_iter()
                    .map(|m| self.clone_stmt(arena, m))
                    .collect();
                Stmt::ActorDecl {
                    name,
                    generic_params,
                    fields: new_fields,
                    methods: new_methods,
                    implements,
                }
            }
            Stmt::StructDecl {
                name,
                generic_params,
                fields,
            } => {
                let new_fields = fields
                    .into_iter()
                    .map(|f| self.clone_stmt(arena, f))
                    .collect();
                Stmt::StructDecl {
                    name,
                    generic_params,
                    fields: new_fields,
                }
            }
            Stmt::EnumDecl {
                name,
                generic_params,
                variants,
            } => Stmt::EnumDecl {
                name,
                generic_params,
                variants,
            },
            Stmt::InterfaceDecl {
                name,
                generic_params,
                methods,
            } => {
                let new_methods = methods
                    .into_iter()
                    .map(|m| self.clone_stmt(arena, m))
                    .collect();
                Stmt::InterfaceDecl {
                    name,
                    generic_params,
                    methods: new_methods,
                }
            }
            Stmt::VarDecl {
                name,
                is_mutable,
                type_annotation,
                is_static,
                visibility,
                is_weak,
                initializer,
                span,
            } => {
                let new_initializer = initializer.map(|expr| self.clone_expr(arena, expr));
                Stmt::VarDecl {
                    name,
                    is_mutable,
                    type_annotation,
                    is_static,
                    is_weak,
                    visibility,
                    initializer: new_initializer,
                    span,
                }
            }
            Stmt::Expr(expr) => Stmt::Expr(self.clone_expr(arena, expr)),
            Stmt::Return(expr) => Stmt::Return(expr.map(|e| self.clone_expr(arena, e))),
            Stmt::Block(stmts) => Stmt::Block(
                stmts
                    .into_iter()
                    .map(|s| self.clone_stmt(arena, s))
                    .collect(),
            ),
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => Stmt::If {
                condition: self.clone_expr(arena, condition),
                then_branch: self.clone_stmt(arena, then_branch),
                else_branch: else_branch.map(|eb| self.clone_stmt(arena, eb)),
            },
            Stmt::While { condition, body } => Stmt::While {
                condition: self.clone_expr(arena, condition),
                body: self.clone_stmt(arena, body),
            },
            Stmt::Loop { body } => Stmt::Loop {
                body: self.clone_stmt(arena, body),
            },
            Stmt::ForIn {
                item,
                iterable,
                body,
            } => Stmt::ForIn {
                item,
                iterable: self.clone_expr(arena, iterable),
                body: self.clone_stmt(arena, body),
            },
            Stmt::Match { expr, arms } => {
                let new_arms = arms
                    .into_iter()
                    .map(|(p, s)| (p, self.clone_stmt(arena, s)))
                    .collect();
                Stmt::Match {
                    expr: self.clone_expr(arena, expr),
                    arms: new_arms,
                }
            }
            Stmt::Export { .. } | Stmt::Import { .. } => stmt,
        };
        arena.alloc_stmt(new_stmt, pace_ast::Span::default())
    }

    fn clone_expr(
        &mut self,
        arena: &mut pace_ast::arena::AstArena,
        expr_id: pace_ast::arena::ExprId,
    ) -> pace_ast::arena::ExprId {
        let expr = arena.get_expr(expr_id).clone();
        let new_expr = match expr {
            Expr::Call { callee, args } => Expr::Call {
                callee: self.clone_expr(arena, callee),
                args: args
                    .into_iter()
                    .map(|a| self.clone_expr(arena, a))
                    .collect(),
            },
            Expr::Unary {
                op,
                expr: inner_expr,
            } => Expr::Unary {
                op,
                expr: self.clone_expr(arena, inner_expr),
            },
            Expr::Binary { left, op, right } => Expr::Binary {
                left: self.clone_expr(arena, left),
                op,
                right: self.clone_expr(arena, right),
            },
            Expr::Assign { target, value } => Expr::Assign {
                target: self.clone_expr(arena, target),
                value: self.clone_expr(arena, value),
            },
            Expr::MemberAccess {
                object,
                property,
                computed_class,
                is_static_operator,
            } => Expr::MemberAccess {
                object: self.clone_expr(arena, object),
                property,
                computed_class,
                is_static_operator,
            },
            Expr::OptionalMemberAccess { object, property } => Expr::OptionalMemberAccess {
                object: self.clone_expr(arena, object),
                property,
            },
            Expr::Block(stmts) => Expr::Block(
                stmts
                    .into_iter()
                    .map(|s| self.clone_stmt(arena, s))
                    .collect(),
            ),
            Expr::GenericInstantiation {
                callee,
                generic_args,
            } => Expr::GenericInstantiation {
                callee: self.clone_expr(arena, callee),
                generic_args,
            },
            Expr::Closure {
                params,
                return_type,
                body,
            } => Expr::Closure {
                params,
                return_type,
                body: self.clone_expr(arena, body),
            },
            Expr::InterpolatedString(parts) => Expr::InterpolatedString(
                parts
                    .into_iter()
                    .map(|p| self.clone_expr(arena, p))
                    .collect(),
            ),
            Expr::Unwrap(expr) => Expr::Unwrap(self.clone_expr(arena, expr)),
            Expr::NullCoalesce { left, right } => Expr::NullCoalesce {
                left: self.clone_expr(arena, left),
                right: self.clone_expr(arena, right),
            },
            Expr::Try(expr) => Expr::Try(self.clone_expr(arena, expr)),
            Expr::Await(expr) => Expr::Await(self.clone_expr(arena, expr)),
            Expr::Identifier(..)
            | Expr::IntLiteral(..)
            | Expr::FloatLiteral(..)
            | Expr::StringLiteral(..)
            | Expr::BoolLiteral(..)
            | Expr::Null => expr,
        };
        arena.alloc_expr(new_expr, pace_ast::Span::default())
    }

    fn instantiate_class(
        &mut self,
        arena: &mut pace_ast::arena::AstArena,
        generic_decl_id: pace_ast::arena::StmtId,
        concrete_name: String,
        concrete_args: &[TypeAnnotation],
    ) -> Result<(), miette::Report> {
        // Prevent infinite recursion by inserting a dummy first
        let dummy_expr = arena.alloc_expr(Expr::Null, pace_ast::Span::default());
        let dummy_id = arena.alloc_stmt(Stmt::Expr(dummy_expr), pace_ast::Span::default());
        self.generated_classes
            .insert(concrete_name.clone().into(), dummy_id);

        let generic_decl = arena.get_stmt(generic_decl_id).clone();
        let is_actor = matches!(generic_decl, Stmt::ActorDecl { .. });
        match generic_decl {
            Stmt::ClassDecl {
                name: _,
                generic_params,
                fields,
                methods,
                implements,
                ..
            }
            | Stmt::ActorDecl {
                name: _,
                generic_params,
                fields,
                methods,
                implements,
                ..
            } => {
                let params = generic_params.unwrap();
                let mut type_mapping = std::collections::HashMap::new();
                for (i, p) in params.iter().enumerate() {
                    if let Some(arg) = concrete_args.get(i) {
                        type_mapping.insert(*p, arg.clone());
                    }
                }

                let mut new_fields = Vec::new();
                for &f_id in &fields {
                    let new_f_id = self.clone_stmt(arena, f_id);
                    self.rewrite_stmt_with_mapping(arena, new_f_id, &type_mapping)?;
                    new_fields.push(new_f_id);
                }

                let mut new_methods = Vec::new();
                for &m_id in &methods {
                    let new_m_id = self.clone_stmt(arena, m_id);
                    self.rewrite_stmt_with_mapping(arena, new_m_id, &type_mapping)?;
                    new_methods.push(new_m_id);
                }

                let mut new_implements = implements;
                if let Some(imp) = &mut new_implements {
                    self.replace_types(imp, &type_mapping)?;
                    self.rewrite_type_annotation(arena, imp)?;

                    // Inject default methods from the interface if not overridden by the class
                    let iface_decl_opt = self
                        .generated_classes
                        .get(&imp.name)
                        .or_else(|| self.all_interfaces.get(&imp.name))
                        .cloned();
                    if let Some(iface_decl_id) = iface_decl_opt
                        && let Stmt::InterfaceDecl {
                            methods: iface_methods,
                            ..
                        } = arena.get_stmt(iface_decl_id).clone()
                    {
                        for iface_method_id in iface_methods {
                            if let Stmt::FuncDecl {
                                name: iface_m_name,
                                body: iface_m_body,
                                ..
                            } = arena.get_stmt(iface_method_id).clone()
                            {
                                if iface_m_body.is_empty() {
                                    continue;
                                }
                                let already_implemented = new_methods.iter().any(|&m_id| {
                                    if let Stmt::FuncDecl {
                                        name: cls_m_name, ..
                                    } = arena.get_stmt(m_id)
                                    {
                                        cls_m_name == &iface_m_name
                                    } else {
                                        false
                                    }
                                });
                                if !already_implemented {
                                    let cloned_iface_method_id =
                                        self.clone_stmt(arena, iface_method_id);
                                    new_methods.push(cloned_iface_method_id);
                                }
                            }
                        }
                    }
                }

                let instantiated = if is_actor {
                    Stmt::ActorDecl {
                        name: concrete_name.clone().into(),
                        generic_params: None, // It's concrete now!
                        fields: new_fields,
                        methods: new_methods,
                        implements: new_implements,
                    }
                } else {
                    Stmt::ClassDecl {
                        name: concrete_name.clone().into(),
                        generic_params: None, // It's concrete now!
                        fields: new_fields,
                        methods: new_methods,
                        implements: new_implements,
                    }
                };
                *arena.get_stmt_mut(dummy_id) = instantiated;
            }
            Stmt::StructDecl {
                name: _,
                generic_params,
                fields,
                ..
            } => {
                let params = generic_params.unwrap();
                let mut type_mapping = std::collections::HashMap::new();
                for (i, p) in params.iter().enumerate() {
                    if let Some(arg) = concrete_args.get(i) {
                        type_mapping.insert(*p, arg.clone());
                    }
                }
                let mut new_fields = Vec::new();
                for &f_id in &fields {
                    let new_f_id = self.clone_stmt(arena, f_id);
                    self.rewrite_stmt_with_mapping(arena, new_f_id, &type_mapping)?;
                    new_fields.push(new_f_id);
                }
                let instantiated = Stmt::StructDecl {
                    name: concrete_name.clone().into(),
                    generic_params: None,
                    fields: new_fields,
                };
                *arena.get_stmt_mut(dummy_id) = instantiated;
            }
            Stmt::EnumDecl {
                name: _,
                generic_params,
                variants,
                ..
            } => {
                let params = generic_params.unwrap();
                let mut type_mapping = std::collections::HashMap::new();
                for (i, p) in params.iter().enumerate() {
                    if let Some(arg) = concrete_args.get(i) {
                        type_mapping.insert(*p, arg.clone());
                    }
                }
                let mut new_variants = Vec::new();
                for v in variants {
                    let mut new_fields = None;
                    if let Some(fields) = &v.fields {
                        let mut nf = Vec::new();
                        for f in fields {
                            let mut new_f = f.clone();
                            self.replace_types(&mut new_f, &type_mapping)?;
                            self.rewrite_type_annotation(arena, &mut new_f)?;
                            nf.push(new_f);
                        }
                        new_fields = Some(nf);
                    }
                    new_variants.push(pace_ast::EnumVariant {
                        name: v.name,
                        fields: new_fields,
                    });
                }
                let instantiated = Stmt::EnumDecl {
                    name: concrete_name.clone().into(),
                    generic_params: None,
                    variants: new_variants,
                };
                *arena.get_stmt_mut(dummy_id) = instantiated;
            }
            Stmt::InterfaceDecl {
                name: _,
                generic_params,
                methods,
                ..
            } => {
                let params = generic_params.unwrap();
                let mut type_mapping = std::collections::HashMap::new();
                for (i, p) in params.iter().enumerate() {
                    if let Some(arg) = concrete_args.get(i) {
                        type_mapping.insert(*p, arg.clone());
                    }
                }
                let mut new_methods = Vec::new();
                for &m_id in &methods {
                    let new_m_id = self.clone_stmt(arena, m_id);
                    self.rewrite_stmt_with_mapping(arena, new_m_id, &type_mapping)?;
                    new_methods.push(new_m_id);
                }
                let instantiated = Stmt::InterfaceDecl {
                    name: concrete_name.clone().into(),
                    generic_params: None,
                    methods: new_methods,
                };
                *arena.get_stmt_mut(dummy_id) = instantiated;
            }
            Stmt::FuncDecl {
                name: _,
                generic_params,
                params,
                return_type,
                body,
                is_async,
                is_static,
                is_extern,
                visibility,
                span,
            } => {
                let gen_params = generic_params.unwrap();
                let mut type_mapping = std::collections::HashMap::new();
                for (i, p) in gen_params.iter().enumerate() {
                    if let Some(arg) = concrete_args.get(i) {
                        type_mapping.insert(*p, arg.clone());
                    }
                }

                let mut new_params = Vec::new();
                for p in params {
                    let mut new_p = p.clone();
                    self.replace_types(&mut new_p.type_annotation, &type_mapping)?;
                    self.rewrite_type_annotation(arena, &mut new_p.type_annotation)?;
                    new_params.push(new_p);
                }

                let mut new_return_type = return_type.clone();
                if let Some(ty) = &mut new_return_type {
                    self.replace_types(ty, &type_mapping)?;
                    self.rewrite_type_annotation(arena, ty)?;
                }

                let mut new_body = Vec::new();
                for &s_id in &body {
                    let new_s_id = self.clone_stmt(arena, s_id);
                    self.rewrite_stmt_with_mapping(arena, new_s_id, &type_mapping)?;
                    new_body.push(new_s_id);
                }

                let instantiated = Stmt::FuncDecl {
                    name: concrete_name.clone().into(),
                    generic_params: None,
                    params: new_params,
                    return_type: new_return_type,
                    body: new_body,
                    is_async,
                    is_static,
                    is_extern,
                    visibility,
                    span,
                };
                *arena.get_stmt_mut(dummy_id) = instantiated;
            }
            _ => {}
        }
        Ok(())
    }

    fn replace_types(
        &mut self,
        ty: &mut TypeAnnotation,
        mapping: &HashMap<ustr::Ustr, TypeAnnotation>,
    ) -> Result<(), miette::Report> {
        if let Some(mapped) = mapping.get(&ty.name) {
            *ty = mapped.clone();
        }
        for arg in &mut ty.args {
            self.replace_types(arg, mapping)?;
        }
        Ok(())
    }

    fn rewrite_stmt_with_mapping(
        &mut self,
        arena: &mut pace_ast::arena::AstArena,
        stmt_id: pace_ast::arena::StmtId,
        mapping: &std::collections::HashMap<ustr::Ustr, TypeAnnotation>,
    ) -> Result<(), miette::Report> {
        // First substitute types
        self.substitute_stmt_types(arena, stmt_id, mapping)?;
        // Then recursively rewrite
        self.rewrite_stmt(arena, stmt_id)
    }

    fn substitute_stmt_types(
        &mut self,
        arena: &mut pace_ast::arena::AstArena,
        stmt_id: pace_ast::arena::StmtId,
        mapping: &std::collections::HashMap<ustr::Ustr, TypeAnnotation>,
    ) -> Result<(), miette::Report> {
        let mut stmt = arena.get_stmt(stmt_id).clone();
        match &mut stmt {
            Stmt::VarDecl {
                type_annotation,
                initializer,
                ..
            } => {
                if let Some(ty) = type_annotation {
                    self.replace_types(ty, mapping)?;
                    self.rewrite_type_annotation(arena, ty)?;
                }
                if let Some(expr) = initializer {
                    self.substitute_expr_types(arena, *expr, mapping)?;
                }
            }
            Stmt::FuncDecl {
                params,
                return_type,
                body,
                ..
            } => {
                for p in params {
                    self.replace_types(&mut p.type_annotation, mapping)?;
                    self.rewrite_type_annotation(arena, &mut p.type_annotation)?;
                }
                if let Some(ty) = return_type {
                    self.replace_types(ty, mapping)?;
                    self.rewrite_type_annotation(arena, ty)?;
                }
                for s in body {
                    self.substitute_stmt_types(arena, *s, mapping)?;
                }
            }
            Stmt::ClassDecl {
                fields,
                methods,
                implements,
                ..
            }
            | Stmt::ActorDecl {
                fields,
                methods,
                implements,
                ..
            } => {
                for f in fields {
                    self.substitute_stmt_types(arena, *f, mapping)?;
                }
                for m in methods {
                    self.substitute_stmt_types(arena, *m, mapping)?;
                }
                if let Some(imp) = implements {
                    self.replace_types(imp, mapping)?;
                    self.rewrite_type_annotation(arena, imp)?;
                }
            }
            Stmt::InterfaceDecl { methods, .. } => {
                for m in methods {
                    self.substitute_stmt_types(arena, *m, mapping)?;
                }
            }
            Stmt::EnumDecl { variants, .. } => {
                for v in variants {
                    if let Some(fields) = &mut v.fields {
                        for ty in fields {
                            self.replace_types(ty, mapping)?;
                            self.rewrite_type_annotation(arena, ty)?;
                        }
                    }
                }
            }
            Stmt::Match { expr, arms } => {
                self.substitute_expr_types(arena, *expr, mapping)?;
                for (_pattern, body) in arms {
                    self.substitute_stmt_types(arena, *body, mapping)?;
                }
            }
            Stmt::Expr(expr) => self.substitute_expr_types(arena, *expr, mapping)?,
            Stmt::Return(expr) => {
                if let Some(e) = expr {
                    self.substitute_expr_types(arena, *e, mapping)?;
                }
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.substitute_expr_types(arena, *condition, mapping)?;
                self.substitute_stmt_types(arena, *then_branch, mapping)?;
                if let Some(eb) = else_branch {
                    self.substitute_stmt_types(arena, *eb, mapping)?;
                }
            }
            Stmt::While { condition, body } => {
                self.substitute_expr_types(arena, *condition, mapping)?;
                self.substitute_stmt_types(arena, *body, mapping)?;
            }
            Stmt::Block(stmts) => {
                for s in stmts {
                    self.substitute_stmt_types(arena, *s, mapping)?;
                }
            }
            Stmt::Module { body, .. } => {
                for s in body {
                    self.substitute_stmt_types(arena, *s, mapping)?;
                }
            }
            Stmt::StructDecl { fields, .. } => {
                for f in fields {
                    self.substitute_stmt_types(arena, *f, mapping)?;
                }
            }
            _ => {}
        }
        *arena.get_stmt_mut(stmt_id) = stmt;
        Ok(())
    }

    fn substitute_expr_types(
        &mut self,
        arena: &mut pace_ast::arena::AstArena,
        expr_id: pace_ast::arena::ExprId,
        mapping: &std::collections::HashMap<ustr::Ustr, pace_ast::TypeAnnotation>,
    ) -> Result<(), miette::Report> {
        let mut expr = arena.get_expr(expr_id).clone();
        match &mut expr {
            Expr::GenericInstantiation {
                callee,
                generic_args,
            } => {
                self.substitute_expr_types(arena, *callee, mapping)?;
                for arg in generic_args.iter_mut() {
                    self.replace_types(arg, mapping)?;
                    self.rewrite_type_annotation(arena, arg)?;
                }
            }
            Expr::Call { callee, args } => {
                self.substitute_expr_types(arena, *callee, mapping)?;
                for arg in args {
                    self.substitute_expr_types(arena, *arg, mapping)?;
                }
            }
            Expr::Binary { left, right, .. } => {
                self.substitute_expr_types(arena, *left, mapping)?;
                self.substitute_expr_types(arena, *right, mapping)?;
            }
            Expr::Assign { target, value } => {
                self.substitute_expr_types(arena, *target, mapping)?;
                self.substitute_expr_types(arena, *value, mapping)?;
            }
            Expr::MemberAccess {
                object,
                property: _,
                computed_class: _,
                is_static_operator: _,
            } => {
                self.substitute_expr_types(arena, *object, mapping)?;
            }
            Expr::OptionalMemberAccess { object, .. } => {
                self.substitute_expr_types(arena, *object, mapping)?;
            }
            _ => {}
        }
        *arena.get_expr_mut(expr_id) = expr;
        Ok(())
    }

    fn rewrite_stmt(
        &mut self,
        arena: &mut pace_ast::arena::AstArena,
        stmt_id: pace_ast::arena::StmtId,
    ) -> Result<(), miette::Report> {
        let mut stmt = arena.get_stmt(stmt_id).clone();

        match &mut stmt {
            Stmt::ClassDecl { .. } | Stmt::ActorDecl { .. } => {}
            Stmt::FuncDecl { .. } => {}
            _ => {}
        }

        match &mut stmt {
            Stmt::VarDecl {
                type_annotation,
                initializer,
                ..
            } => {
                if let Some(ty) = type_annotation {
                    self.rewrite_type_annotation(arena, ty)?;
                }
                if let Some(expr) = initializer {
                    let ex = arena.get_expr(*expr).clone();
                    if let pace_ast::expr::Expr::Call { .. } = &ex {}
                    self.rewrite_expr(arena, *expr)?;
                }
            }
            Stmt::FuncDecl {
                params,
                return_type,
                body,
                ..
            } => {
                for p in params {
                    self.rewrite_type_annotation(arena, &mut p.type_annotation)?;
                }
                if let Some(ty) = return_type {
                    self.rewrite_type_annotation(arena, ty)?;
                }
                for s in body {
                    self.rewrite_stmt(arena, *s)?;
                }
            }
            Stmt::ClassDecl {
                fields,
                methods,
                implements,
                ..
            }
            | Stmt::ActorDecl {
                fields,
                methods,
                implements,
                ..
            } => {
                for f in fields.iter_mut() {
                    self.rewrite_stmt(arena, *f)?;
                }
                for m in methods.iter_mut() {
                    self.rewrite_stmt(arena, *m)?;
                }
                if let Some(imp) = implements {
                    self.rewrite_type_annotation(arena, imp)?;

                    // Inject default methods for concrete classes
                    let iface_decl_opt = self
                        .generated_classes
                        .get(&imp.name)
                        .or_else(|| self.all_interfaces.get(&imp.name))
                        .cloned();
                    if let Some(iface_decl_id) = iface_decl_opt
                        && let Stmt::InterfaceDecl {
                            methods: iface_methods,
                            ..
                        } = arena.get_stmt(iface_decl_id).clone()
                    {
                        for iface_method_id in iface_methods {
                            if let Stmt::FuncDecl {
                                name: iface_m_name,
                                body: iface_m_body,
                                ..
                            } = arena.get_stmt(iface_method_id).clone()
                            {
                                if iface_m_body.is_empty() {
                                    continue;
                                }
                                let already_implemented = methods.iter().any(|&m_id| {
                                    if let Stmt::FuncDecl {
                                        name: cls_m_name, ..
                                    } = arena.get_stmt(m_id)
                                    {
                                        cls_m_name == &iface_m_name
                                    } else {
                                        false
                                    }
                                });
                                if !already_implemented {
                                    methods.push(iface_method_id);
                                }
                            }
                        }
                    }
                }
            }
            Stmt::InterfaceDecl { methods, .. } => {
                for m in methods {
                    self.rewrite_stmt(arena, *m)?;
                }
            }
            Stmt::EnumDecl { variants, .. } => {
                for v in variants {
                    if let Some(fields) = &mut v.fields {
                        for ty in fields {
                            self.rewrite_type_annotation(arena, ty)?;
                        }
                    }
                }
            }
            Stmt::Match { expr, arms } => {
                self.rewrite_expr(arena, *expr)?;
                for (pattern, body) in arms {
                    self.rewrite_pattern(arena, pattern)?;
                    self.rewrite_stmt(arena, *body)?;
                }
            }
            Stmt::Expr(expr) => self.rewrite_expr(arena, *expr)?,
            Stmt::Return(expr) => {
                if let Some(e) = expr {
                    self.rewrite_expr(arena, *e)?;
                }
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.rewrite_expr(arena, *condition)?;
                self.rewrite_stmt(arena, *then_branch)?;
                if let Some(eb) = else_branch {
                    self.rewrite_stmt(arena, *eb)?;
                }
            }
            Stmt::While { condition, body } => {
                self.rewrite_expr(arena, *condition)?;
                self.rewrite_stmt(arena, *body)?;
            }
            Stmt::Block(stmts) => {
                for s in stmts {
                    self.rewrite_stmt(arena, *s)?;
                }
            }
            Stmt::Module { body, .. } => {
                for s in body.iter_mut() {
                    self.rewrite_stmt(arena, *s)?;
                }
            }
            Stmt::StructDecl { fields, .. } => {
                for f in fields.iter_mut() {
                    self.rewrite_stmt(arena, *f)?;
                }
            }
            _ => {}
        }
        *arena.get_stmt_mut(stmt_id) = stmt;
        Ok(())
    }

    fn rewrite_expr(
        &mut self,
        arena: &mut pace_ast::arena::AstArena,
        expr_id: pace_ast::arena::ExprId,
    ) -> Result<(), miette::Report> {
        let mut expr = arena.get_expr(expr_id).clone();
        match &mut expr {
            Expr::GenericInstantiation {
                callee,
                generic_args,
            } => {
                self.rewrite_expr(arena, *callee)?;
                for arg in generic_args.iter_mut() {
                    self.rewrite_type_annotation(arena, arg)?;
                }

                if let Expr::Identifier(name, _) = arena.get_expr(*callee) {
                    if name.as_str() == "__pace_retain_generic"
                        || name.as_str() == "__pace_release_generic"
                    {
                        let type_name = generic_args[0].name.as_str();
                        let is_primitive = matches!(
                            type_name,
                            "Int" | "Float" | "Bool" | "Char" | "Byte" | "Void" | "String"
                        );
                        if is_primitive {
                            // We shouldn't hit this normally because Expr::Call intercepts it,
                            // but just in case, we leave it as a dummy identifier
                            expr = Expr::Identifier(
                                ustr::Ustr::from("__pace_noop"),
                                pace_ast::Span::default(),
                            );
                        } else {
                            let target_func = if name.as_str() == "__pace_retain_generic" {
                                "retain"
                            } else {
                                "release"
                            };
                            expr = Expr::Identifier(
                                ustr::Ustr::from(target_func),
                                pace_ast::Span::default(),
                            );
                        }
                        *arena.get_expr_mut(expr_id) = expr;
                        return Ok(());
                    }

                    // Convert to a simple identifier and instantiate the class
                    let concrete_name = Self::generate_name(name, generic_args);
                    if !self
                        .generated_classes
                        .contains_key(&ustr::Ustr::from(&concrete_name))
                        && !self
                            .generated_funcs
                            .contains_key(&ustr::Ustr::from(&concrete_name))
                    {
                        let mut found = self.generic_classes.get(name).cloned();
                        if found.is_none() {
                            found = self.generic_funcs.get(name).cloned();
                        }
                        if found.is_none() {
                            let target_suffix = format!("_{}", name.as_str());
                            let target_suffix_2 = format!("__{}", name.as_str());
                            for (k, v) in
                                self.generic_classes.iter().chain(self.generic_funcs.iter())
                            {
                                let k_str = k.as_str();
                                if k_str.ends_with(&target_suffix)
                                    || k_str.ends_with(&target_suffix_2)
                                    || k_str == name.as_str()
                                {
                                    found = Some(*v);
                                    break;
                                }
                            }
                        }
                        if let Some(generic_decl) = found {
                            let is_func =
                                matches!(arena.get_stmt(generic_decl), Stmt::FuncDecl { .. });
                            if is_func {
                                let dummy_expr_id = arena.alloc_expr(Expr::Null, pace_ast::Span::default());
                                let dummy_id = arena.alloc_stmt(Stmt::Expr(dummy_expr_id), pace_ast::Span::default());
                                self.generated_funcs
                                    .insert(concrete_name.clone().into(), dummy_id); // Dummy to prevent recursion
                            }
                            let _ = self.instantiate_class(
                                arena,
                                generic_decl,
                                concrete_name.clone(),
                                generic_args,
                            );
                        }
                    }
                    expr = Expr::Identifier(concrete_name.into(), pace_ast::Span::default());
                }
            }
            Expr::Call { callee, args } => {
                if let Expr::GenericInstantiation {
                    callee: inner_callee,
                    generic_args,
                } = arena.get_expr(*callee)
                    && let Expr::Identifier(name, _) = arena.get_expr(*inner_callee)
                    && (name.as_str() == "__pace_retain_generic"
                        || name.as_str() == "__pace_release_generic")
                {
                    let type_name = generic_args[0].name.as_str();
                    let is_primitive = matches!(
                        type_name,
                        "Int" | "Float" | "Bool" | "Char" | "Byte" | "Void" | "String"
                    );
                    if is_primitive {
                        expr = Expr::IntLiteral(0);
                        *arena.get_expr_mut(expr_id) = expr;
                        return Ok(());
                    }
                }

                self.rewrite_expr(arena, *callee)?;
                for arg in args {
                    self.rewrite_expr(arena, *arg)?;
                }
            }
            Expr::Binary { left, right, .. } => {
                self.rewrite_expr(arena, *left)?;
                self.rewrite_expr(arena, *right)?;
            }
            Expr::Assign { target, value } => {
                self.rewrite_expr(arena, *target)?;
                self.rewrite_expr(arena, *value)?;
            }
            Expr::MemberAccess {
                object,
                property: _,
                computed_class: _,
                is_static_operator: _,
            } => {
                self.rewrite_expr(arena, *object)?;
            }
            _ => {}
        }
        
        *arena.get_expr_mut(expr_id) = expr;
        Ok(())
    }

    fn rewrite_pattern(
        &mut self,
        arena: &mut pace_ast::arena::AstArena,
        pat: &mut pace_ast::Pattern,
    ) -> Result<(), miette::Report> {
        if let pace_ast::Pattern::Variant {
            enum_name,
            variant_name: _,
            fields,
            generic_args,
        } = pat
        {
            if let Some(args) = generic_args {
                for arg in args.iter_mut() {
                    self.rewrite_type_annotation(arena, arg)?;
                }
                if let Some(name) = enum_name {
                    let concrete_name = Self::generate_name(name, args);
                    *name = concrete_name.into();
                }
                *generic_args = None;
            }
            if let Some(flds) = fields {
                for f in flds {
                    self.rewrite_pattern(arena, f)?;
                }
            }
        }
        Ok(())
    }
}
