use pace_ast::{Expr, Param, Stmt, TypeAnnotation};
use std::collections::{HashMap, HashSet};

pub struct Monomorphizer<'a> {
    pub arena: &'a mut pace_ast::arena::AstArena,
    pub type_replacements: HashMap<ustr::Ustr, TypeAnnotation>,
}

impl<'a> Monomorphizer<'a> {
    pub fn new(arena: &'a mut pace_ast::arena::AstArena, replacements: HashMap<ustr::Ustr, TypeAnnotation>) -> Self {
        Self {
            arena,
            type_replacements: replacements,
        }
    }

    
    pub fn instantiate_stmt(&mut self, stmt_id: pace_ast::arena::StmtId) -> pace_ast::arena::StmtId {
        let stmt = self.arena.get_stmt(stmt_id).clone();
        let new_stmt = self.instantiate_stmt_inner(&stmt);
        self.arena.alloc_stmt(new_stmt)
    }

    pub fn instantiate_expr(&mut self, expr_id: pace_ast::arena::ExprId) -> pace_ast::arena::ExprId {
        let expr = self.arena.get_expr(expr_id).clone();
        let new_expr = self.instantiate_expr_inner(&expr);
        self.arena.alloc_expr(new_expr)
    }

    pub fn instantiate_stmt_inner(&mut self, stmt: &Stmt) -> Stmt {
        match stmt {
            Stmt::Expr(expr) => Stmt::Expr(self.instantiate_expr(*expr)),
            Stmt::VarDecl {
                name,
                is_mutable,
                type_annotation,
                is_static,
                visibility,
                initializer,
                span,
            } => Stmt::VarDecl {
                name: *name,
                is_mutable: *is_mutable,
                type_annotation: type_annotation
                    .as_ref()
                    .map(|t| self.instantiate_type_annotation(t)),
                is_static: *is_static,
                visibility: visibility.clone(),
                initializer: initializer.map(|e| self.instantiate_expr(e)),
                span: *span,
            },
            Stmt::Block(stmts) => {
                Stmt::Block(stmts.iter().map(|s| self.instantiate_stmt(*s)).collect())
            }
            Stmt::Return(expr) => Stmt::Return(expr.map(|e| self.instantiate_expr(e))),
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => Stmt::If {
                condition: self.instantiate_expr(*condition),
                then_branch: self.instantiate_stmt(*then_branch),
                else_branch: else_branch
                    .as_ref()
                    .map(|b| self.instantiate_stmt(*b)),
            },
            Stmt::While { condition, body } => Stmt::While {
                condition: self.instantiate_expr(*condition),
                body: self.instantiate_stmt(*body),
            },
            Stmt::Loop { body } => Stmt::Loop {
                body: self.instantiate_stmt(*body),
            },
            Stmt::FuncDecl {
                name,
                generic_params: _,
                params,
                return_type,
                body,
                is_async,
                is_static,
                visibility,
                doc_comment,
                span,
            } => {
                Stmt::FuncDecl {
                    name: *name,
                    generic_params: None, // Monomorphized functions are no longer generic
                    params: params
                        .iter()
                        .map(|p| Param {
                            name: p.name,
                            type_annotation: self.instantiate_type_annotation(&p.type_annotation),
                        })
                        .collect(),
                    return_type: return_type
                        .as_ref()
                        .map(|t| self.instantiate_type_annotation(t)),
                    body: body.iter().map(|s| self.instantiate_stmt(*s)).collect(),
                    is_async: *is_async,
                    is_static: *is_static,
                    visibility: visibility.clone(),
                    doc_comment: *doc_comment,
                    span: *span,
                }
            }
            Stmt::ClassDecl {
                name,
                generic_params: _,
                fields,
                methods,
                implements,
                doc_comment,
            } => Stmt::ClassDecl {
                name: *name,
                generic_params: None,
                fields: fields.iter().map(|f| self.instantiate_stmt(*f)).collect(),
                methods: methods.iter().map(|m| self.instantiate_stmt(*m)).collect(),
                implements: implements
                    .as_ref()
                    .map(|t| self.instantiate_type_annotation(t)),
                doc_comment: *doc_comment,
            },
            Stmt::StructDecl {
                name,
                generic_params: _,
                fields,
                doc_comment,
            } => Stmt::StructDecl {
                name: *name,
                generic_params: None,
                fields: fields.iter().map(|f| self.instantiate_stmt(*f)).collect(),
                doc_comment: *doc_comment,
            },
            Stmt::InterfaceDecl {
                name,
                generic_params: _,
                methods,
                doc_comment,
            } => Stmt::InterfaceDecl {
                name: *name,
                generic_params: None,
                methods: methods.iter().map(|m| self.instantiate_stmt(*m)).collect(),
                doc_comment: *doc_comment,
            },
            _ => stmt.clone(),
        }
    }

    pub fn instantiate_expr_inner(&mut self, expr: &Expr) -> Expr {
        match expr {
            Expr::Binary { left, op, right } => Expr::Binary {
                left: self.instantiate_expr(*left),
                op: op.clone(),
                right: self.instantiate_expr(*right),
            },
            Expr::Call { callee, args } => Expr::Call {
                callee: self.instantiate_expr(*callee),
                args: args.iter().map(|a| self.instantiate_expr(*a)).collect(),
            },
            Expr::Assign { target, value } => Expr::Assign {
                target: self.instantiate_expr(*target),
                value: self.instantiate_expr(*value),
            },
            Expr::MemberAccess {
                object,
                property,
                computed_class,
                is_static_operator,
            } => Expr::MemberAccess {
                object: self.instantiate_expr(*object),
                property: *property,
                computed_class: *computed_class,
                is_static_operator: *is_static_operator,
            },
            Expr::GenericInstantiation {
                callee,
                generic_args,
            } => Expr::GenericInstantiation {
                callee: self.instantiate_expr(*callee),
                generic_args: generic_args
                    .iter()
                    .map(|a| self.instantiate_type_annotation(a))
                    .collect(),
            },
            _ => expr.clone(),
        }
    }

    pub fn instantiate_type_annotation(&self, type_ann: &TypeAnnotation) -> TypeAnnotation {
        if let Some(replacement) = self.type_replacements.get(&type_ann.name) {
            let mut new_ann = replacement.clone();
            new_ann.is_nullable = new_ann.is_nullable || type_ann.is_nullable;
            new_ann
        } else {
            TypeAnnotation {
                module_prefix: type_ann.module_prefix,
                name: type_ann.name,
                args: type_ann
                    .args
                    .iter()
                    .map(|a| self.instantiate_type_annotation(a))
                    .collect(),
                is_nullable: type_ann.is_nullable,
                is_function: false,
                function_params: None,
                function_return: None,
            }
        }
    }
}

pub struct MonomorphizationPass<'a> {
    pub arena: &'a mut pace_ast::arena::AstArena,
    templates: HashMap<ustr::Ustr, pace_ast::arena::StmtId>, // Name -> AST Node
    pub final_stmts: Vec<pace_ast::arena::StmtId>,
    queue: Vec<(String, Vec<TypeAnnotation>)>, // template_name, args
    instantiated: HashSet<String>,             // specialized names like "Box_Int"
}

// removed default


impl<'a> MonomorphizationPass<'a> {
    pub fn new(arena: &'a mut pace_ast::arena::AstArena) -> Self {
        Self {
            arena,
            templates: HashMap::new(),
            final_stmts: Vec::new(),
            queue: Vec::new(),
            instantiated: HashSet::new(),
        }
    }

    pub fn process(&mut self, stmts: &[pace_ast::arena::StmtId]) {
        let mut non_generics = Vec::new();

        // Pass 1: Extract templates
        for stmt_id in stmts {
            let stmt = self.arena.get_stmt(*stmt_id);
            let (is_generic, name) = match stmt {
                Stmt::ClassDecl {
                    name,
                    generic_params,
                    ..
                } => (generic_params.is_some(), Some(name)),
                Stmt::StructDecl {
                    name,
                    generic_params,
                    ..
                } => (generic_params.is_some(), Some(name)),
                Stmt::InterfaceDecl {
                    name,
                    generic_params,
                    ..
                } => (generic_params.is_some(), Some(name)),
                Stmt::FuncDecl {
                    name,
                    generic_params,
                    ..
                } => (generic_params.is_some(), Some(name)),
                _ => (false, None),
            };

            if is_generic {
                if let Some(n) = name {
                    self.templates.insert(*n, *stmt_id);
                }
            } else {
                non_generics.push(*stmt_id);
            }
        }

        // Pass 2: Monomorphize non-generics and scan for generics
        for stmt in non_generics {
            let stmt_ref = self.arena.get_stmt(stmt).clone();
            self.scan_stmt(&stmt_ref);
            self.final_stmts.push(stmt);
        }

        // Pass 3: Process the queue
        while let Some((template_name, args)) = self.queue.pop() {
            let specialized_name = format!(
                "{}_{}",
                template_name,
                args.iter()
                    .map(Self::flatten_type_name)
                    .collect::<Vec<_>>()
                    .join("_")
            );

            if self.instantiated.contains(&specialized_name) {
                continue;
            }
            self.instantiated.insert(specialized_name.clone());

            if let Some(template) = self.templates.get(&ustr::Ustr::from(&template_name)).cloned() {
                let mut replacements = HashMap::new();
                let generic_params = match self.arena.get_stmt(template) {
                    Stmt::ClassDecl { generic_params, .. } => generic_params.clone(),
                    Stmt::StructDecl { generic_params, .. } => generic_params.clone(),
                    Stmt::InterfaceDecl { generic_params, .. } => generic_params.clone(),
                    Stmt::FuncDecl { generic_params, .. } => generic_params.clone(),
                    _ => None,
                }
                .unwrap_or_default();

                for (i, param_name) in generic_params.iter().enumerate() {
                    if i < args.len() {
                        replacements.insert(*param_name, args[i].clone());
                    }
                }

                let mut mono = Monomorphizer::new(self.arena, replacements);
                let instantiated_stmt = mono.instantiate_stmt(template);

                // Set the specialized name
                match self.arena.get_stmt_mut(instantiated_stmt) {
                    Stmt::ClassDecl { name, .. } => *name = specialized_name.clone().into(),
                    Stmt::StructDecl { name, .. } => *name = specialized_name.clone().into(),
                    Stmt::InterfaceDecl { name, .. } => *name = specialized_name.clone().into(),
                    Stmt::FuncDecl { name, .. } => *name = specialized_name.clone().into(),
                    _ => {}
                }

                // Scan the newly instantiated statement for more generics
                let stmt_ref = self.arena.get_stmt(instantiated_stmt).clone();
                self.scan_stmt(&stmt_ref);

                self.final_stmts.push(instantiated_stmt);
            }
        }

        // Pass 4: Flatten all type annotations in final_stmts
        let mut flattener = TypeFlattener { arena: self.arena };
        self.final_stmts = self
            .final_stmts
            .iter()
            .map(|s| flattener.flatten_stmt(*s))
            .collect();
    }

    pub fn flatten_type_name(ta: &TypeAnnotation) -> String {
        if ta.args.is_empty() {
            ta.name.to_string()
        } else {
            let args_str: Vec<String> = ta.args.iter().map(Self::flatten_type_name).collect();
            format!("{}_{}", ta.name, args_str.join("_"))
        }
    }

    fn scan_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::VarDecl {
                type_annotation, ..
            } => {
                if let Some(ta) = type_annotation {
                    self.scan_type_annotation(ta);
                }
            }
            Stmt::FuncDecl {
                params,
                return_type,
                body,
                ..
            } => {
                for p in params {
                    self.scan_type_annotation(&p.type_annotation);
                }
                if let Some(rt) = return_type {
                    self.scan_type_annotation(rt);
                }
                for s in body {
                    { let __stmt = self.arena.get_stmt(*s).clone(); self.scan_stmt(&__stmt); }
                }
            }
            Stmt::ClassDecl {
                fields,
                methods,
                implements,
                ..
            } => {
                for f in fields {
                    { let __stmt = self.arena.get_stmt(*f).clone(); self.scan_stmt(&__stmt); }
                }
                for m in methods {
                    { let __stmt = self.arena.get_stmt(*m).clone(); self.scan_stmt(&__stmt); }
                }
                if let Some(imp) = implements {
                    self.scan_type_annotation(imp);
                }
            }
            Stmt::StructDecl { fields, .. } => {
                for f in fields {
                    { let __stmt = self.arena.get_stmt(*f).clone(); self.scan_stmt(&__stmt); }
                }
            }
            Stmt::InterfaceDecl { methods, .. } => {
                for m in methods {
                    { let __stmt = self.arena.get_stmt(*m).clone(); self.scan_stmt(&__stmt); }
                }
            }
            Stmt::Block(stmts) => {
                for s in stmts {
                    { let __stmt = self.arena.get_stmt(*s).clone(); self.scan_stmt(&__stmt); }
                }
            }
            _ => {} // Expressions could contain GenericInstantiation
        }
    }

    fn scan_type_annotation(&mut self, ta: &TypeAnnotation) {
        if !ta.args.is_empty() {
            // Found a generic usage!
            self.queue.push((ustr::Ustr::from(&ta.name).to_string(), ta.args.clone()));
            for arg in &ta.args {
                self.scan_type_annotation(arg);
            }
        }
    }
}

pub struct TypeFlattener<'a> {
    pub arena: &'a mut pace_ast::arena::AstArena,
}

impl<'a> TypeFlattener<'a> {
    pub fn flatten_stmt(&mut self, stmt_id: pace_ast::arena::StmtId) -> pace_ast::arena::StmtId {
        let stmt = self.arena.get_stmt(stmt_id).clone();
        let new_stmt = self.flatten_stmt_inner(&stmt);
        self.arena.alloc_stmt(new_stmt)
    }

    pub fn flatten_expr(&mut self, expr_id: pace_ast::arena::ExprId) -> pace_ast::arena::ExprId {
        let expr = self.arena.get_expr(expr_id).clone();
        let new_expr = self.flatten_expr_inner(&expr);
        self.arena.alloc_expr(new_expr)
    }

    fn flatten_stmt_inner(&mut self, stmt: &Stmt) -> Stmt {
        match stmt {
            Stmt::Expr(expr) => Stmt::Expr(self.flatten_expr(*expr)),
            Stmt::VarDecl {
                name,
                is_mutable,
                type_annotation,
                is_static,
                visibility,
                initializer,
                span,
            } => Stmt::VarDecl {
                name: *name,
                is_mutable: *is_mutable,
                type_annotation: type_annotation
                    .as_ref()
                    .map(|t| self.flatten_type_annotation(t)),
                is_static: *is_static,
                visibility: visibility.clone(),
                initializer: initializer.as_ref().map(|e| self.flatten_expr(*e)),
                span: *span,
            },
            Stmt::Block(stmts) => {
                Stmt::Block(stmts.iter().map(|s| self.flatten_stmt(*s)).collect())
            }
            Stmt::Return(expr) => Stmt::Return(expr.as_ref().map(|e| self.flatten_expr(*e))),
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => Stmt::If {
                condition: self.flatten_expr(*condition),
                then_branch: self.flatten_stmt(*then_branch),
                else_branch: else_branch
                    .as_ref()
                    .map(|b| self.flatten_stmt(*b)),
            },
            Stmt::While { condition, body } => Stmt::While {
                condition: self.flatten_expr(*condition),
                body: self.flatten_stmt(*body),
            },
            Stmt::Loop { body } => Stmt::Loop {
                body: self.flatten_stmt(*body),
            },
            Stmt::FuncDecl {
                name,
                generic_params,
                params,
                return_type,
                body,
                is_async,
                is_static,
                visibility,
                doc_comment,
                span,
            } => Stmt::FuncDecl {
                name: *name,
                generic_params: generic_params.clone(),
                params: params
                    .iter()
                    .map(|p| Param {
                        name: p.name,
                        type_annotation: self.flatten_type_annotation(&p.type_annotation),
                    })
                    .collect(),
                return_type: return_type
                    .as_ref()
                    .map(|t| self.flatten_type_annotation(t)),
                body: body.iter().map(|s| self.flatten_stmt(*s)).collect(),
                is_async: *is_async,
                is_static: *is_static,
                visibility: visibility.clone(),
                doc_comment: *doc_comment,
                span: *span,
            },
            Stmt::ClassDecl {
                name,
                generic_params,
                fields,
                methods,
                implements,
                doc_comment,
            } => Stmt::ClassDecl {
                name: *name,
                generic_params: generic_params.clone(),
                fields: fields.iter().map(|f| self.flatten_stmt(*f)).collect(),
                methods: methods.iter().map(|m| self.flatten_stmt(*m)).collect(),
                implements: implements.as_ref().map(|t| self.flatten_type_annotation(t)),
                doc_comment: *doc_comment,
            },
            Stmt::StructDecl {
                name,
                generic_params,
                fields,
                doc_comment,
            } => Stmt::StructDecl {
                name: *name,
                generic_params: generic_params.clone(),
                fields: fields.iter().map(|f| self.flatten_stmt(*f)).collect(),
                doc_comment: *doc_comment,
            },
            Stmt::InterfaceDecl {
                name,
                generic_params,
                methods,
                doc_comment,
            } => Stmt::InterfaceDecl {
                name: *name,
                generic_params: generic_params.clone(),
                methods: methods.iter().map(|m| self.flatten_stmt(*m)).collect(),
                doc_comment: *doc_comment,
            },
            _ => stmt.clone(),
        }
    }

    fn flatten_expr_inner(&mut self, expr: &Expr) -> Expr {
        match expr {
            Expr::Binary { left, op, right } => Expr::Binary {
                left: self.flatten_expr(*left),
                op: op.clone(),
                right: self.flatten_expr(*right),
            },
            Expr::Call { callee, args } => Expr::Call {
                callee: self.flatten_expr(*callee),
                args: args.iter().map(|a| self.flatten_expr(*a)).collect(),
            },
            Expr::Assign { target, value } => Expr::Assign {
                target: self.flatten_expr(*target),
                value: self.flatten_expr(*value),
            },
            Expr::MemberAccess {
                object,
                property,
                computed_class,
                is_static_operator,
            } => Expr::MemberAccess {
                object: self.flatten_expr(*object),
                property: *property,
                computed_class: *computed_class,
                is_static_operator: *is_static_operator,
            },
            Expr::GenericInstantiation {
                callee,
                generic_args,
            } => Expr::GenericInstantiation {
                callee: self.flatten_expr(*callee),
                generic_args: generic_args
                    .iter()
                    .map(|a| self.flatten_type_annotation(a))
                    .collect(),
            },
            _ => expr.clone(),
        }
    }

    fn flatten_type_annotation(&self, ta: &TypeAnnotation) -> TypeAnnotation {
        if ta.args.is_empty() {
            ta.clone()
        } else {
            let name = MonomorphizationPass::flatten_type_name(ta);
            TypeAnnotation {
                module_prefix: ta.module_prefix,
                name: name.into(),
                args: vec![],
                is_nullable: ta.is_nullable,
                is_function: ta.is_function,
                function_params: ta.function_params.clone(),
                function_return: ta.function_return.clone(),
            }
        }
    }
}
