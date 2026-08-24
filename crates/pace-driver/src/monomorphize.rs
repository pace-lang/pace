use pace_ast::{Stmt, Expr, TypeAnnotation};
use std::collections::HashMap;

pub struct Monomorphizer {
    generic_classes: HashMap<String, Stmt>,
    generated_classes: HashMap<String, Stmt>,
}

impl Monomorphizer {
    pub fn new() -> Self {
        Self {
            generic_classes: HashMap::new(),
            generated_classes: HashMap::new(),
        }
    }

    pub fn run(ast: Vec<Stmt>) -> Result<Vec<Stmt>, miette::Report> {
        let mut mono = Self::new();
        let mut final_ast = Vec::new();
        // Pass 1: Extract all generic class and interface declarations
        for stmt in ast {
            if let Stmt::ClassDecl { name, generic_params, .. } = &stmt {
                if generic_params.is_some() {
                    mono.generic_classes.insert(name.clone(), stmt.clone());
                    continue;
                }
            }
            if let Stmt::InterfaceDecl { name, generic_params, .. } = &stmt {
                if generic_params.is_some() {
                    mono.generic_classes.insert(name.clone(), stmt.clone());
                    continue;
                }
            }
            if let Stmt::Module { name, body } = stmt {
                let mut new_body = Vec::new();
                for inner_stmt in body {
                    if let Stmt::ClassDecl { name: cname, generic_params, .. } = &inner_stmt {
                        if generic_params.is_some() {
                            mono.generic_classes.insert(cname.clone(), inner_stmt.clone());
                            continue;
                        }
                    }
                    if let Stmt::InterfaceDecl { name: iname, generic_params, .. } = &inner_stmt {
                        if generic_params.is_some() {
                            mono.generic_classes.insert(iname.clone(), inner_stmt.clone());
                            continue;
                        }
                    }
                    new_body.push(inner_stmt);
                }
                final_ast.push(Stmt::Module { name, body: new_body });
                continue;
            }
            final_ast.push(stmt);
        }

        // Pass 2: Rewrite AST and instantiate generics
        let mut rewritten_ast = Vec::new();
        for stmt in final_ast {
            if let Stmt::Module { name, body } = stmt {
                let mut new_body = Vec::new();
                for inner_stmt in body {
                    new_body.push(mono.rewrite_stmt(inner_stmt)?);
                }
                rewritten_ast.push(Stmt::Module { name, body: new_body });
            } else {
                rewritten_ast.push(mono.rewrite_stmt(stmt)?);
            }
        }
        
        // Append all newly generated classes to the end of the AST
        let mut generated: Vec<Stmt> = mono.generated_classes.into_values().collect();
        if let Some(Stmt::Module { body, name: _ }) = rewritten_ast.last_mut() {
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
            
            if !self.generated_classes.contains_key(&concrete_name) {
                if let Some(generic_decl) = self.generic_classes.get(&ty.name).cloned() {
                    self.instantiate_class(generic_decl, concrete_name.clone(), &ty.args)?;
                } else {
                    // It might be a built-in or something not found
                }
            }
            
            ty.name = concrete_name;
            ty.args.clear();
        }
        Ok(())
    }

    fn instantiate_class(&mut self, generic_decl: Stmt, concrete_name: String, concrete_args: &[TypeAnnotation]) -> Result<(), miette::Report> {
        // Prevent infinite recursion by inserting a dummy first
        self.generated_classes.insert(concrete_name.clone(), Stmt::Expr(Expr::Null));

        match generic_decl {
            Stmt::ClassDecl { name: _, generic_params, fields, methods, implements } => {
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
                }

                let instantiated = Stmt::ClassDecl {
                    name: concrete_name.clone(),
                    generic_params: None, // It's concrete now!
                    fields: new_fields,
                    methods: new_methods,
                    implements: new_implements,
                };
                self.generated_classes.insert(concrete_name, instantiated);
            }
            Stmt::InterfaceDecl { name: _, generic_params, methods } => {
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
                    name: concrete_name.clone(),
                    generic_params: None, // It's concrete now!
                    methods: new_methods,
                };
                self.generated_classes.insert(concrete_name, instantiated);
            }
            _ => {}
        }
        Ok(())
    }
    
    fn replace_types(&mut self, ty: &mut TypeAnnotation, mapping: &HashMap<String, TypeAnnotation>) -> Result<(), miette::Report> {
        if let Some(mapped) = mapping.get(&ty.name) {
            *ty = mapped.clone();
        }
        for arg in &mut ty.args {
            self.replace_types(arg, mapping)?;
        }
        Ok(())
    }

    fn rewrite_stmt_with_mapping(&mut self, mut stmt: Stmt, mapping: &HashMap<String, TypeAnnotation>) -> Result<Stmt, miette::Report> {
        // First substitute types
        self.substitute_stmt_types(&mut stmt, mapping)?;
        // Then recursively rewrite
        self.rewrite_stmt(stmt)
    }

    fn substitute_stmt_types(&mut self, stmt: &mut Stmt, mapping: &HashMap<String, TypeAnnotation>) -> Result<(), miette::Report> {
        match stmt {
            Stmt::VarDecl { type_annotation, initializer, .. } => {
                if let Some(ty) = type_annotation {
                    self.replace_types(ty, mapping)?;
                }
                if let Some(expr) = initializer {
                    self.substitute_expr_types(expr, mapping)?;
                }
            }
            Stmt::FuncDecl { params, return_type, body, .. } => {
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
            Stmt::ClassDecl { fields, methods, implements, .. } => {
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
            Stmt::If { condition, then_branch, else_branch } => {
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

    fn substitute_expr_types(&mut self, expr: &mut Expr, mapping: &HashMap<String, TypeAnnotation>) -> Result<(), miette::Report> {
        match expr {
            Expr::GenericInstantiation { callee, generic_args } => {
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
            Expr::MemberAccess { object, .. } => {
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
            Stmt::VarDecl { type_annotation, initializer, .. } => {
                if let Some(ty) = type_annotation {
                    self.rewrite_type_annotation(ty)?;
                }
                if let Some(expr) = initializer {
                    self.rewrite_expr(expr)?;
                }
            }
            Stmt::FuncDecl { params, return_type, body, .. } => {
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
            Stmt::ClassDecl { fields, methods, implements, .. } => {
                for f in fields {
                    *f = self.rewrite_stmt(f.clone())?;
                }
                for m in methods {
                    *m = self.rewrite_stmt(m.clone())?;
                }
                if let Some(imp) = implements {
                    self.rewrite_type_annotation(imp)?;
                }
            }
            Stmt::InterfaceDecl { methods, .. } => {
                for m in methods {
                    *m = self.rewrite_stmt(m.clone())?;
                }
            }
            Stmt::Expr(expr) => self.rewrite_expr(expr)?,
            Stmt::Return(expr) => {
                if let Some(e) = expr {
                    self.rewrite_expr(e)?;
                }
            }
            Stmt::If { condition, then_branch, else_branch } => {
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
            Expr::GenericInstantiation { callee, generic_args } => {
                self.rewrite_expr(callee)?;
                for arg in generic_args.iter_mut() {
                    self.rewrite_type_annotation(arg)?;
                }
                // Convert to a simple identifier
                // Convert to a simple identifier and instantiate the class
                if let Expr::Identifier(name) = &**callee {
                    let concrete_name = Self::generate_name(name, generic_args);
                    if !self.generated_classes.contains_key(&concrete_name) {
                        if let Some(generic_decl) = self.generic_classes.get(name).cloned() {
                            let _ = self.instantiate_class(generic_decl, concrete_name.clone(), generic_args);
                        }
                    }
                    *expr = Expr::Identifier(concrete_name);
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
            Expr::MemberAccess { object, .. } => {
                self.rewrite_expr(object)?;
            }
            _ => {}
        }
        Ok(())
    }
}
