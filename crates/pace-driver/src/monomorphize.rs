use pace_ast::{Expr, Stmt, TypeAnnotation};
use std::collections::HashMap;

pub struct Monomorphizer {
    pub generic_classes: HashMap<ustr::Ustr, Stmt>,
    pub generic_class_modules: HashMap<ustr::Ustr, String>,
    pub generated_classes: HashMap<ustr::Ustr, Stmt>,
    pub all_interfaces: HashMap<ustr::Ustr, Stmt>, // Stores all interfaces (generic and concrete)
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
            all_interfaces: HashMap::new(),
        }
    }

    pub fn run(ast: Vec<Stmt>) -> Result<Vec<Stmt>, miette::Report> {
        let mut mono = Self::new();
        let mut final_ast = Vec::new();
        // Pass 1: Extract all generic class and interface declarations
        for stmt in ast {
            if let Stmt::ClassDecl {
                name,
                generic_params,
                ..
            }
            | Stmt::ActorDecl {
                name,
                generic_params,
                ..
            } = &stmt
                && generic_params.is_some()
            {
                mono.generic_classes.insert(name.clone(), stmt.clone());
                continue;
            }
            if let Stmt::InterfaceDecl {
                name,
                generic_params,
                ..
            } = &stmt
            {
                mono.all_interfaces.insert(name.clone(), stmt.clone());
                if generic_params.is_some() {
                    mono.generic_classes.insert(name.clone(), stmt.clone());
                    continue;
                }
            }
            if let Stmt::EnumDecl {
                name,
                generic_params,
                ..
            } = &stmt
                && generic_params.is_some()
            {
                mono.generic_classes.insert(name.clone(), stmt.clone());
                continue;
            }
            if let Stmt::Module { name, body } = stmt {
                let mut new_body = Vec::new();
                for inner_stmt in body {
                    if let Stmt::ClassDecl {
                        name: cname,
                        generic_params,
                        ..
                    }
                    | Stmt::ActorDecl {
                        name: cname,
                        generic_params,
                        ..
                    } = &inner_stmt
                        && generic_params.is_some()
                    {
                        mono.generic_classes
                            .insert(cname.clone(), inner_stmt.clone());
                        mono.generic_class_modules
                            .insert(cname.clone(), name.to_string());
                        continue;
                    }
                    if let Stmt::InterfaceDecl {
                        name: iname,
                        generic_params,
                        ..
                    } = &inner_stmt
                    {
                        mono.all_interfaces
                            .insert(iname.clone(), inner_stmt.clone());
                        if generic_params.is_some() {
                            mono.generic_classes
                                .insert(iname.clone(), inner_stmt.clone());
                            mono.generic_class_modules
                                .insert(iname.clone(), name.to_string());
                            continue;
                        }
                    }
                    if let Stmt::EnumDecl {
                        name: ename,
                        generic_params,
                        ..
                    } = &inner_stmt
                        && generic_params.is_some()
                    {
                        mono.generic_classes
                            .insert(ename.clone(), inner_stmt.clone());
                        mono.generic_class_modules
                            .insert(ename.clone(), name.to_string());
                        continue;
                    }
                    new_body.push(inner_stmt);
                }
                final_ast.push(Stmt::Module {
                    name,
                    body: new_body,
                });
            } else {
                final_ast.push(stmt);
            }
        }

        // Pass 2: Rewrite AST and instantiate generics
        let mut rewritten_ast = Vec::new();
        for stmt in final_ast {
            if let Stmt::Module { name, body } = stmt {
                let mut new_body = Vec::new();
                for inner_stmt in body {
                    new_body.push(mono.rewrite_stmt(inner_stmt)?);
                }
                rewritten_ast.push(Stmt::Module {
                    name,
                    body: new_body,
                });
            } else {
                rewritten_ast.push(mono.rewrite_stmt(stmt)?);
            }
        }

        // Append all newly generated classes to the end of the AST
        let mut generated = Vec::new();
        for (concrete_name, instantiated_stmt) in mono.generated_classes {
            // Find original generic name (strip _TypeArgs...)
            let base_name = concrete_name
                .split('_')
                .next()
                .unwrap_or(&concrete_name)
                .to_string();
            let original_module = mono
                .generic_class_modules
                .get(&ustr::Ustr::from(&base_name))
                .cloned()
                .unwrap_or_else(|| "unknown_module".to_string());

            generated.push(Stmt::Module {
                name: original_module.into(),
                body: vec![instantiated_stmt],
            });
        }

        if let Some(Stmt::Module { body, .. }) = rewritten_ast.last_mut() {
            body.append(&mut generated);
        } else {
            rewritten_ast.append(&mut generated);
        }

        Ok(rewritten_ast)
    }

    fn generate_name(base: &str, args: &[TypeAnnotation]) -> String {
        let mut name = base.to_string();
        for arg in args {
            name.push('_');
            name.push_str(&Self::generate_name(&arg.name, &arg.args));
        }
        name
    }

    fn rewrite_type_annotation(&mut self, ty: &mut TypeAnnotation) -> Result<(), miette::Report> {
        if !ty.args.is_empty() {
            for arg in &mut ty.args {
                self.rewrite_type_annotation(arg)?;
            }

            let concrete_name = Self::generate_name(&ty.name, &ty.args);

            if !self.generated_classes.contains_key(&ustr::Ustr::from(&concrete_name)) {
                if let Some(generic_decl) = self.generic_classes.get(&ty.name).cloned() {
                    self.instantiate_class(generic_decl, concrete_name.clone(), &ty.args)?;
                } else {
                    // It might be a built-in or something not found
                }
            }

            ty.name = concrete_name.into();
            ty.args.clear();
        }
        Ok(())
    }

    fn instantiate_class(
        &mut self,
        generic_decl: Stmt,
        concrete_name: String,
        concrete_args: &[TypeAnnotation],
    ) -> Result<(), miette::Report> {
        // Prevent infinite recursion by inserting a dummy first
        self.generated_classes
            .insert(concrete_name.clone().into(), Stmt::Expr(Expr::Null));

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
                let mut type_mapping = HashMap::new();
                for (i, p) in params.iter().enumerate() {
                    if let Some(arg) = concrete_args.get(i) {
                        type_mapping.insert(p.clone(), arg.clone());
                    }
                }

                let mut new_fields = Vec::new();
                for f in fields {
                    new_fields.push(self.rewrite_stmt_with_mapping(f, &type_mapping)?);
                }

                let mut new_methods = Vec::new();
                for m in methods {
                    new_methods.push(self.rewrite_stmt_with_mapping(m, &type_mapping)?);
                }

                let mut new_implements = implements;
                if let Some(imp) = &mut new_implements {
                    self.replace_types(imp, &type_mapping)?;
                    self.rewrite_type_annotation(imp)?;

                    // Inject default methods from the interface if not overridden by the class
                    let iface_decl_opt = self
                        .generated_classes
                        .get(&imp.name)
                        .or_else(|| self.all_interfaces.get(&imp.name))
                        .cloned();
                    if let Some(Stmt::InterfaceDecl {
                        methods: iface_methods,
                        ..
                    }) = iface_decl_opt
                    {
                        for iface_method in iface_methods {
                            if let Stmt::FuncDecl {
                                name: iface_m_name,
                                body: iface_m_body,
                                ..
                            } = &iface_method
                                && !iface_m_body.is_empty()
                            {
                                let already_implemented = new_methods.iter().any(|m| {
                                    if let Stmt::FuncDecl {
                                        name: cls_m_name, ..
                                    } = m
                                    {
                                        cls_m_name == iface_m_name
                                    } else {
                                        false
                                    }
                                });
                                if !already_implemented {
                                    new_methods.push(iface_method.clone());
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
                        doc_comment: None,
                    }
                } else {
                    Stmt::ClassDecl {
                        name: concrete_name.clone().into(),
                        generic_params: None, // It's concrete now!
                        fields: new_fields,
                        methods: new_methods,
                        implements: new_implements,
                        doc_comment: None,
                    }
                };
                self.generated_classes.insert(concrete_name.into(), instantiated);
            }
            Stmt::InterfaceDecl {
                name: _,
                generic_params,
                methods,
                ..
            } => {
                let params = generic_params.unwrap();
                let mut type_mapping = HashMap::new();
                for (i, p) in params.iter().enumerate() {
                    if let Some(arg) = concrete_args.get(i) {
                        type_mapping.insert(p.clone(), arg.clone());
                    }
                }

                let mut new_methods = Vec::new();
                for m in methods {
                    new_methods.push(self.rewrite_stmt_with_mapping(m, &type_mapping)?);
                }

                let instantiated = Stmt::InterfaceDecl {
                    name: concrete_name.clone().into(),
                    generic_params: None, // It's concrete now!
                    methods: new_methods,
                    doc_comment: None,
                };
                self.generated_classes.insert(concrete_name.into(), instantiated);
            }
            Stmt::EnumDecl {
                name: _,
                generic_params,
                variants,
                ..
            } => {
                let params = generic_params.unwrap();
                let mut type_mapping = HashMap::new();
                for (i, p) in params.iter().enumerate() {
                    if let Some(arg) = concrete_args.get(i) {
                        type_mapping.insert(p.clone(), arg.clone());
                    }
                }

                let mut new_variants = Vec::new();
                for v in variants {
                    let mut new_v = v.clone();
                    if let Some(fields) = &mut new_v.fields {
                        for field_ty in fields {
                            self.replace_types(field_ty, &type_mapping)?;
                            self.rewrite_type_annotation(field_ty)?;
                        }
                    }
                    new_variants.push(new_v);
                }

                let instantiated = Stmt::EnumDecl {
                    name: concrete_name.clone().into(),
                    generic_params: None, // It's concrete now!
                    variants: new_variants,
                    doc_comment: None,
                };
                self.generated_classes.insert(concrete_name.into(), instantiated);
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
        mut stmt: Stmt,
        mapping: &HashMap<ustr::Ustr, TypeAnnotation>,
    ) -> Result<Stmt, miette::Report> {
        // First substitute types
        self.substitute_stmt_types(&mut stmt, mapping)?;
        // Then recursively rewrite
        self.rewrite_stmt(stmt)
    }

    fn substitute_stmt_types(
        &mut self,
        stmt: &mut Stmt,
        mapping: &HashMap<ustr::Ustr, TypeAnnotation>,
    ) -> Result<(), miette::Report> {
        match stmt {
            Stmt::VarDecl {
                type_annotation,
                initializer,
                ..
            } => {
                if let Some(ty) = type_annotation {
                    self.replace_types(ty, mapping)?;
                }
                if let Some(expr) = initializer {
                    self.substitute_expr_types(expr, mapping)?;
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
                }
                if let Some(ty) = return_type {
                    self.replace_types(ty, mapping)?;
                }
                for s in body {
                    self.substitute_stmt_types(s, mapping)?;
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
                    self.substitute_stmt_types(f, mapping)?;
                }
                for m in methods {
                    self.substitute_stmt_types(m, mapping)?;
                }
                if let Some(imp) = implements {
                    self.replace_types(imp, mapping)?;
                }
            }
            Stmt::Expr(expr) => self.substitute_expr_types(expr, mapping)?,
            Stmt::Return(expr) => {
                if let Some(e) = expr {
                    self.substitute_expr_types(e, mapping)?;
                }
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.substitute_expr_types(condition, mapping)?;
                self.substitute_stmt_types(then_branch, mapping)?;
                if let Some(eb) = else_branch {
                    self.substitute_stmt_types(eb, mapping)?;
                }
            }
            Stmt::While { condition, body } => {
                self.substitute_expr_types(condition, mapping)?;
                self.substitute_stmt_types(body, mapping)?;
            }
            Stmt::Block(stmts) => {
                for s in stmts {
                    self.substitute_stmt_types(s, mapping)?;
                }
            }
            Stmt::InterfaceDecl { methods, .. } => {
                for m in methods {
                    self.substitute_stmt_types(m, mapping)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn substitute_expr_types(
        &mut self,
        expr: &mut Expr,
        mapping: &HashMap<ustr::Ustr, TypeAnnotation>,
    ) -> Result<(), miette::Report> {
        match expr {
            Expr::GenericInstantiation {
                callee,
                generic_args,
            } => {
                self.substitute_expr_types(callee, mapping)?;
                for arg in generic_args {
                    self.replace_types(arg, mapping)?;
                }
            }
            Expr::Call { callee, args } => {
                self.substitute_expr_types(callee, mapping)?;
                for arg in args {
                    self.substitute_expr_types(arg, mapping)?;
                }
            }
            Expr::Binary { left, right, .. } => {
                self.substitute_expr_types(left, mapping)?;
                self.substitute_expr_types(right, mapping)?;
            }
            Expr::Assign { target, value } => {
                self.substitute_expr_types(target, mapping)?;
                self.substitute_expr_types(value, mapping)?;
            }
            Expr::MemberAccess {
                object,
                property: _,
                computed_class: _,
                is_static_operator: _,
            } => {
                self.substitute_expr_types(object, mapping)?;
            }
            Expr::OptionalMemberAccess { object, .. } => {
                self.substitute_expr_types(object, mapping)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn rewrite_stmt(&mut self, mut stmt: Stmt) -> Result<Stmt, miette::Report> {
        match &mut stmt {
            Stmt::VarDecl {
                type_annotation,
                initializer,
                ..
            } => {
                if let Some(ty) = type_annotation {
                    self.rewrite_type_annotation(ty)?;
                }
                if let Some(expr) = initializer {
                    self.rewrite_expr(expr)?;
                }
            }
            Stmt::FuncDecl {
                params,
                return_type,
                body,
                ..
            } => {
                for p in params {
                    self.rewrite_type_annotation(&mut p.type_annotation)?;
                }
                if let Some(ty) = return_type {
                    self.rewrite_type_annotation(ty)?;
                }
                for s in body {
                    *s = self.rewrite_stmt(s.clone())?;
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
                    *f = self.rewrite_stmt(f.clone())?;
                }
                for m in methods.iter_mut() {
                    *m = self.rewrite_stmt(m.clone())?;
                }
                if let Some(imp) = implements {
                    self.rewrite_type_annotation(imp)?;

                    // Inject default methods for concrete classes
                    let iface_decl_opt = self
                        .generated_classes
                        .get(&imp.name)
                        .or_else(|| self.all_interfaces.get(&imp.name))
                        .cloned();
                    if let Some(Stmt::InterfaceDecl {
                        methods: iface_methods,
                        ..
                    }) = iface_decl_opt
                    {
                        for iface_method in iface_methods {
                            if let Stmt::FuncDecl {
                                name: iface_m_name,
                                body: iface_m_body,
                                ..
                            } = &iface_method
                                && !iface_m_body.is_empty()
                            {
                                let already_implemented = methods.iter().any(|m| {
                                    if let Stmt::FuncDecl {
                                        name: cls_m_name, ..
                                    } = m
                                    {
                                        cls_m_name == iface_m_name
                                    } else {
                                        false
                                    }
                                });
                                if !already_implemented {
                                    methods.push(iface_method.clone());
                                }
                            }
                        }
                    }
                }
            }
            Stmt::InterfaceDecl { methods, .. } => {
                for m in methods {
                    *m = self.rewrite_stmt(m.clone())?;
                }
            }
            Stmt::EnumDecl { variants, .. } => {
                for v in variants {
                    if let Some(fields) = &mut v.fields {
                        for ty in fields {
                            self.rewrite_type_annotation(ty)?;
                        }
                    }
                }
            }
            Stmt::Match { expr, arms } => {
                self.rewrite_expr(expr)?;
                for (pattern, body) in arms {
                    self.rewrite_pattern(pattern)?;
                    **body = self.rewrite_stmt(*body.clone())?;
                }
            }
            Stmt::Expr(expr) => self.rewrite_expr(expr)?,
            Stmt::Return(expr) => {
                if let Some(e) = expr {
                    self.rewrite_expr(e)?;
                }
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.rewrite_expr(condition)?;
                **then_branch = self.rewrite_stmt(*then_branch.clone())?;
                if let Some(eb) = else_branch {
                    **eb = self.rewrite_stmt(*eb.clone())?;
                }
            }
            Stmt::While { condition, body } => {
                self.rewrite_expr(condition)?;
                **body = self.rewrite_stmt(*body.clone())?;
            }
            Stmt::Block(stmts) => {
                for s in stmts {
                    *s = self.rewrite_stmt(s.clone())?;
                }
            }
            _ => {}
        }
        Ok(stmt)
    }

    fn rewrite_expr(&mut self, expr: &mut Expr) -> Result<(), miette::Report> {
        match expr {
            Expr::GenericInstantiation {
                callee,
                generic_args,
            } => {
                self.rewrite_expr(callee)?;
                for arg in generic_args.iter_mut() {
                    self.rewrite_type_annotation(arg)?;
                }
                // Convert to a simple identifier
                // Convert to a simple identifier and instantiate the class
                if let Expr::Identifier(name, _) = &**callee {
                    let concrete_name = Self::generate_name(name, generic_args);
                    if !self.generated_classes.contains_key(&ustr::Ustr::from(&concrete_name))
                        && let Some(generic_decl) = self.generic_classes.get(name).cloned()
                    {
                        let _ = self.instantiate_class(
                            generic_decl,
                            concrete_name.clone(),
                            generic_args,
                        );
                    }
                    *expr = Expr::Identifier(concrete_name.into(), pace_ast::Span::default());
                }
            }
            Expr::Call { callee, args } => {
                self.rewrite_expr(callee)?;
                for arg in args {
                    self.rewrite_expr(arg)?;
                }
            }
            Expr::Binary { left, right, .. } => {
                self.rewrite_expr(left)?;
                self.rewrite_expr(right)?;
            }
            Expr::Assign { target, value } => {
                self.rewrite_expr(target)?;
                self.rewrite_expr(value)?;
            }
            Expr::MemberAccess {
                object,
                property: _,
                computed_class: _,
                is_static_operator: _,
            } => {
                self.rewrite_expr(object)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn rewrite_pattern(&mut self, pat: &mut pace_ast::Pattern) -> Result<(), miette::Report> {
        if let pace_ast::Pattern::Variant {
            enum_name,
            variant_name: _,
            fields,
            generic_args,
        } = pat
        {
            if let Some(args) = generic_args {
                for arg in args.iter_mut() {
                    self.rewrite_type_annotation(arg)?;
                }
                if let Some(name) = enum_name {
                    let concrete_name = Self::generate_name(name, args);
                    *name = concrete_name.into();
                }
                *generic_args = None;
            }
            if let Some(flds) = fields {
                for f in flds {
                    self.rewrite_pattern(f)?;
                }
            }
        }
        Ok(())
    }
}
