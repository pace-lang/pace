use pace_ast::{Stmt, Expr, TypeAnnotation, Param};
use std::collections::{HashMap, HashSet};

pub struct Monomorphizer {
    pub type_replacements: HashMap<String, TypeAnnotation>,
}

impl Monomorphizer {
    pub fn new(replacements: HashMap<String, TypeAnnotation>) -> Self {
        Self {
            type_replacements: replacements,
        }
    }

    pub fn instantiate_stmt(&self, stmt: &Stmt) -> Stmt {
        match stmt {
            Stmt::Expr(expr) => Stmt::Expr(self.instantiate_expr(expr)),
            Stmt::VarDecl { name, is_mutable, type_annotation, is_static, visibility, initializer, span } => {
                Stmt::VarDecl {
                    name: name.clone(),
                    is_mutable: *is_mutable,
                    type_annotation: type_annotation.as_ref().map(|t| self.instantiate_type_annotation(t)),
                    is_static: *is_static,
                    visibility: visibility.clone(),
                    initializer: initializer.as_ref().map(|e| self.instantiate_expr(e)),
                    span: *span,
                }
            }
            Stmt::Block(stmts) => Stmt::Block(stmts.iter().map(|s| self.instantiate_stmt(s)).collect()),
            Stmt::Return(expr) => Stmt::Return(expr.as_ref().map(|e| self.instantiate_expr(e))),
            Stmt::If { condition, then_branch, else_branch } => {
                Stmt::If {
                    condition: self.instantiate_expr(condition),
                    then_branch: Box::new(self.instantiate_stmt(then_branch)),
                    else_branch: else_branch.as_ref().map(|b| Box::new(self.instantiate_stmt(b))),
                }
            }
            Stmt::While { condition, body } => {
                Stmt::While {
                    condition: self.instantiate_expr(condition),
                    body: Box::new(self.instantiate_stmt(body)),
                }
            }
            Stmt::Loop { body } => {
                Stmt::Loop {
                    body: Box::new(self.instantiate_stmt(body)),
                }
            }
            Stmt::FuncDecl { name, generic_params: _, params, return_type, body, is_async, is_static, visibility, doc_comment, span } => {
                Stmt::FuncDecl {
                    name: name.clone(),
                    generic_params: None, // Monomorphized functions are no longer generic
                    params: params.iter().map(|p| Param {
                        name: p.name.clone(),
                        type_annotation: self.instantiate_type_annotation(&p.type_annotation),
                    }).collect(),
                    return_type: return_type.as_ref().map(|t| self.instantiate_type_annotation(t)),
                    body: body.iter().map(|s| self.instantiate_stmt(s)).collect(),
                    is_async: *is_async,
                    is_static: *is_static,
                    visibility: visibility.clone(),
                    doc_comment: doc_comment.clone(),
                    span: *span,
                }
            }
            Stmt::ClassDecl { name, generic_params: _, fields, methods, implements, doc_comment } => {
                Stmt::ClassDecl {
                    name: name.clone(),
                    generic_params: None,
                    fields: fields.iter().map(|f| self.instantiate_stmt(f)).collect(),
                    methods: methods.iter().map(|m| self.instantiate_stmt(m)).collect(),
                    implements: implements.as_ref().map(|t| self.instantiate_type_annotation(t)),
                    doc_comment: doc_comment.clone(),
                }
            }
            Stmt::StructDecl { name, generic_params: _, fields, doc_comment } => {
                Stmt::StructDecl {
                    name: name.clone(),
                    generic_params: None,
                    fields: fields.iter().map(|f| self.instantiate_stmt(f)).collect(),
                    doc_comment: doc_comment.clone(),
                }
            }
            Stmt::InterfaceDecl { name, generic_params: _, methods, doc_comment } => {
                Stmt::InterfaceDecl {
                    name: name.clone(),
                    generic_params: None,
                    methods: methods.iter().map(|m| self.instantiate_stmt(m)).collect(),
                    doc_comment: doc_comment.clone(),
                }
            }
            _ => stmt.clone(),
        }
    }

    pub fn instantiate_expr(&self, expr: &Expr) -> Expr {
        match expr {
            Expr::Binary { left, op, right } => Expr::Binary {
                left: Box::new(self.instantiate_expr(left)),
                op: op.clone(),
                right: Box::new(self.instantiate_expr(right)),
            },
            Expr::Call { callee, args } => Expr::Call {
                callee: Box::new(self.instantiate_expr(callee)),
                args: args.iter().map(|a| self.instantiate_expr(a)).collect(),
            },
            Expr::Assign { target, value } => Expr::Assign {
                target: Box::new(self.instantiate_expr(target)),
                value: Box::new(self.instantiate_expr(value)),
            },
            Expr::MemberAccess { object, property, computed_class, is_static_operator } => Expr::MemberAccess {
                object: Box::new(self.instantiate_expr(object)),
                property: property.clone(),
                computed_class: computed_class.clone(),
                is_static_operator: *is_static_operator,
            },
            Expr::GenericInstantiation { callee, generic_args } => Expr::GenericInstantiation {
                callee: Box::new(self.instantiate_expr(callee)),
                generic_args: generic_args.iter().map(|a| self.instantiate_type_annotation(a)).collect(),
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
                module_prefix: type_ann.module_prefix.clone(),
                name: type_ann.name.clone(),
                args: type_ann.args.iter().map(|a| self.instantiate_type_annotation(a)).collect(),
                is_nullable: type_ann.is_nullable,
                is_function: false,
                function_params: None,
                function_return: None
            }
        }
    }
}

pub struct MonomorphizationPass {
    templates: HashMap<String, Stmt>, // Name -> AST Node
    pub final_stmts: Vec<Stmt>,
    queue: Vec<(String, Vec<TypeAnnotation>)>, // template_name, args
    instantiated: HashSet<String>, // specialized names like "Box_Int"
}

impl Default for MonomorphizationPass {
    fn default() -> Self {
        Self::new()
    }
}

impl MonomorphizationPass {
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
            final_stmts: Vec::new(),
            queue: Vec::new(),
            instantiated: HashSet::new(),
        }
    }

    pub fn process(&mut self, stmts: &[Stmt]) {
        let mut non_generics = Vec::new();
        
        // Pass 1: Extract templates
        for stmt in stmts {
            let (is_generic, name) = match stmt {
                Stmt::ClassDecl { name, generic_params, .. } => (generic_params.is_some(), Some(name)),
                Stmt::StructDecl { name, generic_params, .. } => (generic_params.is_some(), Some(name)),
                Stmt::InterfaceDecl { name, generic_params, .. } => (generic_params.is_some(), Some(name)),
                Stmt::FuncDecl { name, generic_params, .. } => (generic_params.is_some(), Some(name)),
                _ => (false, None),
            };
            
            if is_generic {
                if let Some(n) = name {
                    self.templates.insert(n.clone(), stmt.clone());
                }
            } else {
                non_generics.push(stmt.clone());
            }
        }
        
        // Pass 2: Monomorphize non-generics and scan for generics
        for stmt in non_generics {
            self.scan_stmt(&stmt);
            self.final_stmts.push(stmt);
        }

        // Pass 3: Process the queue
        while let Some((template_name, args)) = self.queue.pop() {
            let specialized_name = format!("{}_{}", template_name, args.iter().map(Self::flatten_type_name).collect::<Vec<_>>().join("_"));
            
            if self.instantiated.contains(&specialized_name) {
                continue;
            }
            self.instantiated.insert(specialized_name.clone());

            if let Some(template) = self.templates.get(&template_name).cloned() {
                let mut replacements = HashMap::new();
                let generic_params = match &template {
                    Stmt::ClassDecl { generic_params, .. } => generic_params.clone(),
                    Stmt::StructDecl { generic_params, .. } => generic_params.clone(),
                    Stmt::InterfaceDecl { generic_params, .. } => generic_params.clone(),
                    Stmt::FuncDecl { generic_params, .. } => generic_params.clone(),
                    _ => None,
                }.unwrap_or_default();

                for (i, param_name) in generic_params.iter().enumerate() {
                    if i < args.len() {
                        replacements.insert(param_name.clone(), args[i].clone());
                    }
                }

                let mono = Monomorphizer::new(replacements);
                let mut instantiated_stmt = mono.instantiate_stmt(&template);

                // Set the specialized name
                match &mut instantiated_stmt {
                    Stmt::ClassDecl { name, .. } => *name = specialized_name.clone(),
                    Stmt::StructDecl { name, .. } => *name = specialized_name.clone(),
                    Stmt::InterfaceDecl { name, .. } => *name = specialized_name.clone(),
                    Stmt::FuncDecl { name, .. } => *name = specialized_name.clone(),
                    _ => {}
                }

                // Scan the newly instantiated statement for more generics
                self.scan_stmt(&instantiated_stmt);
                
                self.final_stmts.push(instantiated_stmt);
            }
        }
        
        // Pass 4: Flatten all type annotations in final_stmts
        let flattener = TypeFlattener {};
        self.final_stmts = self.final_stmts.iter().map(|s| flattener.flatten_stmt(s)).collect();
    }
    
    pub fn flatten_type_name(ta: &TypeAnnotation) -> String {
        if ta.args.is_empty() {
            ta.name.clone()
        } else {
            let args_str: Vec<String> = ta.args.iter().map(Self::flatten_type_name).collect();
            format!("{}_{}", ta.name, args_str.join("_"))
        }
    }
    
    fn scan_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::VarDecl { type_annotation, .. } => {
                if let Some(ta) = type_annotation {
                    self.scan_type_annotation(ta);
                }
            }
            Stmt::FuncDecl { params, return_type, body, .. } => {
                for p in params {
                    self.scan_type_annotation(&p.type_annotation);
                }
                if let Some(rt) = return_type {
                    self.scan_type_annotation(rt);
                }
                for s in body {
                    self.scan_stmt(s);
                }
            }
            Stmt::ClassDecl { fields, methods, implements, .. } => {
                for f in fields {
                    self.scan_stmt(f);
                }
                for m in methods {
                    self.scan_stmt(m);
                }
                if let Some(imp) = implements {
                    self.scan_type_annotation(imp);
                }
            }
            Stmt::StructDecl { fields, .. } => {
                for f in fields {
                    self.scan_stmt(f);
                }
            }
            Stmt::InterfaceDecl { methods, .. } => {
                for m in methods {
                    self.scan_stmt(m);
                }
            }
            Stmt::Block(stmts) => {
                for s in stmts {
                    self.scan_stmt(s);
                }
            }
            _ => {} // Expressions could contain GenericInstantiation
        }
    }
    
    fn scan_type_annotation(&mut self, ta: &TypeAnnotation) {
        if !ta.args.is_empty() {
            // Found a generic usage!
            self.queue.push((ta.name.clone(), ta.args.clone()));
            for arg in &ta.args {
                self.scan_type_annotation(arg);
            }
        }
    }
}

pub struct TypeFlattener {}

impl TypeFlattener {
    pub fn flatten_stmt(&self, stmt: &Stmt) -> Stmt {
        self.do_flatten_stmt(stmt)
    }

    fn do_flatten_stmt(&self, stmt: &Stmt) -> Stmt {
        match stmt {
            Stmt::Expr(expr) => Stmt::Expr(self.flatten_expr(expr)),
            Stmt::VarDecl { name, is_mutable, type_annotation, is_static, visibility, initializer, span } => {
                Stmt::VarDecl {
                    name: name.clone(),
                    is_mutable: *is_mutable,
                    type_annotation: type_annotation.as_ref().map(|t| self.flatten_type_annotation(t)),
                    is_static: *is_static,
                    visibility: visibility.clone(),
                    initializer: initializer.as_ref().map(|e| self.flatten_expr(e)),
                    span: *span,
                }
            }
            Stmt::Block(stmts) => Stmt::Block(stmts.iter().map(|s| self.do_flatten_stmt(s)).collect()),
            Stmt::Return(expr) => Stmt::Return(expr.as_ref().map(|e| self.flatten_expr(e))),
            Stmt::If { condition, then_branch, else_branch } => {
                Stmt::If {
                    condition: self.flatten_expr(condition),
                    then_branch: Box::new(self.do_flatten_stmt(then_branch)),
                    else_branch: else_branch.as_ref().map(|b| Box::new(self.do_flatten_stmt(b))),
                }
            }
            Stmt::While { condition, body } => {
                Stmt::While {
                    condition: self.flatten_expr(condition),
                    body: Box::new(self.do_flatten_stmt(body)),
                }
            }
            Stmt::Loop { body } => {
                Stmt::Loop {
                    body: Box::new(self.do_flatten_stmt(body)),
                }
            }
            Stmt::FuncDecl { name, generic_params, params, return_type, body, is_async, is_static, visibility, doc_comment, span } => {
                Stmt::FuncDecl {
                    name: name.clone(),
                    generic_params: generic_params.clone(),
                    params: params.iter().map(|p| Param {
                        name: p.name.clone(),
                        type_annotation: self.flatten_type_annotation(&p.type_annotation),
                    }).collect(),
                    return_type: return_type.as_ref().map(|t| self.flatten_type_annotation(t)),
                    body: body.iter().map(|s| self.do_flatten_stmt(s)).collect(),
                    is_async: *is_async,
                    is_static: *is_static,
                    visibility: visibility.clone(),
                    doc_comment: doc_comment.clone(),
                    span: *span,
                }
            }
            Stmt::ClassDecl { name, generic_params, fields, methods, implements, doc_comment } => {
                Stmt::ClassDecl {
                    name: name.clone(),
                    generic_params: generic_params.clone(),
                    fields: fields.iter().map(|f| self.do_flatten_stmt(f)).collect(),
                    methods: methods.iter().map(|m| self.do_flatten_stmt(m)).collect(),
                    implements: implements.as_ref().map(|t| self.flatten_type_annotation(t)),
                    doc_comment: doc_comment.clone(),
                }
            }
            Stmt::StructDecl { name, generic_params, fields, doc_comment } => {
                Stmt::StructDecl {
                    name: name.clone(),
                    generic_params: generic_params.clone(),
                    fields: fields.iter().map(|f| self.do_flatten_stmt(f)).collect(),
                    doc_comment: doc_comment.clone(),
                }
            }
            Stmt::InterfaceDecl { name, generic_params, methods, doc_comment } => {
                Stmt::InterfaceDecl {
                    name: name.clone(),
                    generic_params: generic_params.clone(),
                    methods: methods.iter().map(|m| self.do_flatten_stmt(m)).collect(),
                    doc_comment: doc_comment.clone(),
                }
            }
            _ => stmt.clone(),
        }
    }

    fn flatten_expr(&self, expr: &Expr) -> Expr {
        match expr {
            Expr::Binary { left, op, right } => Expr::Binary {
                left: Box::new(self.flatten_expr(left)),
                op: op.clone(),
                right: Box::new(self.flatten_expr(right)),
            },
            Expr::Call { callee, args } => Expr::Call {
                callee: Box::new(self.flatten_expr(callee)),
                args: args.iter().map(|a| self.flatten_expr(a)).collect(),
            },
            Expr::Assign { target, value } => Expr::Assign {
                target: Box::new(self.flatten_expr(target)),
                value: Box::new(self.flatten_expr(value)),
            },
            Expr::MemberAccess { object, property, computed_class, is_static_operator } => Expr::MemberAccess {
                object: Box::new(self.flatten_expr(object)),
                property: property.clone(),
                computed_class: computed_class.clone(),
                is_static_operator: *is_static_operator,
            },
            Expr::GenericInstantiation { callee, generic_args } => Expr::GenericInstantiation {
                callee: Box::new(self.flatten_expr(callee)),
                generic_args: generic_args.iter().map(|a| self.flatten_type_annotation(a)).collect(),
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
                module_prefix: ta.module_prefix.clone(),
                name,
                args: vec![],
                is_nullable: ta.is_nullable,
                is_function: ta.is_function,
                function_params: ta.function_params.clone(),
                function_return: ta.function_return.clone(),
            }
        }
    }
}
