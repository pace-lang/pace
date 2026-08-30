use miette::{Diagnostic, Report, Result};
use pace_ast::{Expr, Stmt, Visibility};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Diagnostic, Debug)]
pub enum ResolutionError {
    #[error("Unresolved symbol: {name}")]
    #[diagnostic(code(pace::unresolved_symbol))]
    UnresolvedSymbol {
        name: String,
        #[label("used here")]
        span: (usize, usize),
    },
    #[error("Cannot access private symbol: {name}")]
    #[diagnostic(code(pace::private_symbol))]
    PrivateSymbol {
        name: String,
        #[label("used here")]
        span: (usize, usize),
    },
    #[error("Collision detected for symbol: {name}. Please use 'as' alias or 'hide'.")]
    #[diagnostic(code(pace::symbol_collision))]
    Collision {
        name: String,
        #[label("used here")]
        span: (usize, usize),
    },
}

#[derive(Clone)]
pub struct ModuleExport {
    pub name: ustr::Ustr,
    pub visibility: Visibility,
    pub mangled_name: String,
}

pub struct SymbolResolver {
    // module_name -> (symbol_name -> export)
    pub exports: HashMap<ustr::Ustr, HashMap<ustr::Ustr, ModuleExport>>,
}

impl SymbolResolver {
    pub fn new() -> Self {
        Self {
            exports: HashMap::new(),
        }
    }

    pub fn run(ast: Vec<Stmt>) -> Result<Vec<Stmt>> {
        let mut resolver = Self::new();
        resolver.resolve(ast)
    }

    pub fn resolve(&mut self, mut ast: Vec<Stmt>) -> Result<Vec<Stmt>> {
        // Pass 1: Collect exports for all modules and mangle their definitions
        for stmt in &mut ast {
            if let Stmt::Module { name, body } = stmt {
                let mut module_exports = HashMap::new();
                for item in body.iter_mut() {
                    let mut is_export = false;
                    let mut vis = Visibility::Public;
                    let mut original_name = String::new();

                    match item {
                        Stmt::FuncDecl {
                            name: n,
                            visibility,
                            ..
                        } => {
                            is_export = true;
                            vis = visibility.clone();
                            original_name = n.to_string();
                        }
                        Stmt::ClassDecl { name: n, .. }
                        | Stmt::ActorDecl { name: n, .. }
                        | Stmt::StructDecl { name: n, .. }
                        | Stmt::EnumDecl { name: n, .. }
                        | Stmt::InterfaceDecl { name: n, .. } => {
                            is_export = true;
                            original_name = n.to_string();
                        }
                        _ => {}
                    }

                    if is_export {
                        let clean_name = name
                            .replace("pkg:", "")
                            .replace("-", "_")
                            .replace("/", "_")
                            .replace(".", "_")
                            .replace(":", "_");
                        let mangled_name = if clean_name == "pace_core" {
                            original_name.clone()
                        } else if original_name == "main" {
                            original_name.clone()
                        } else {
                            format!("{}__{}", clean_name, original_name)
                        };

                        module_exports.insert(
                            original_name.clone().into(),
                            ModuleExport {
                                name: original_name.clone().into(),
                                visibility: vis,
                                mangled_name: mangled_name.clone(),
                            },
                        );

                        match item {
                            Stmt::FuncDecl { name: n, .. } => *n = mangled_name.into(),
                            Stmt::ClassDecl { name: n, .. } | Stmt::ActorDecl { name: n, .. } => {
                                *n = mangled_name.into()
                            }
                            Stmt::StructDecl { name: n, .. } => *n = mangled_name.into(),
                            Stmt::EnumDecl { name: n, .. } => *n = mangled_name.into(),
                            Stmt::InterfaceDecl { name: n, .. } => *n = mangled_name.into(),
                            _ => unreachable!(),
                        }
                    }
                }
                self.exports.insert(name.clone(), module_exports);
            }
        }

        // Pass 2: Resolve references within each module
        for stmt in &mut ast {
            if let Stmt::Module { name, body } = stmt {
                let mut local_scope: HashMap<ustr::Ustr, ModuleExport> = HashMap::new();
                let mut mod_aliases: HashMap<ustr::Ustr, String> = HashMap::new();

                let mut local_declarations: HashMap<ustr::Ustr, ModuleExport> = HashMap::new();
                if let Some(exports) = self.exports.get(name) {
                    for (k, v) in exports {
                        local_declarations.insert(k.clone(), v.clone());
                        local_scope.insert(k.clone(), v.clone());
                    }
                }

                for item in body.iter() {
                    if let Stmt::Import {
                        path,
                        alias,
                        show,
                        hide,
                    } = item
                    {
                        let imported_mod_name = path.clone();
                        if let Some(imported_exports) = self.exports.get(&imported_mod_name) {
                            if let Some(alias_name) = alias {
                                mod_aliases.insert(alias_name.clone(), imported_mod_name.to_string());
                            } else {
                                for (sym, export) in imported_exports {
                                    if export.visibility == Visibility::Private {
                                        continue;
                                    }
                                    if let Some(hide_list) = hide {
                                        if hide_list.contains(sym) {
                                            continue;
                                        }
                                    }
                                    if let Some(show_list) = show {
                                        if !show_list.contains(sym) {
                                            continue;
                                        }
                                    }

                                    if local_declarations.contains_key(sym) {
                                        continue; // Local declarations shadow imports implicitly
                                    }

                                    if let Some(existing) = local_scope.get(sym) {
                                        if existing.mangled_name != export.mangled_name {
                                            local_scope.insert(
                                                sym.clone(),
                                                ModuleExport {
                                                    name: sym.clone(),
                                                    visibility: Visibility::Public,
                                                    mangled_name: "COLLISION".to_string(),
                                                },
                                            );
                                        }
                                    } else {
                                        local_scope.insert(sym.clone(), export.clone());
                                    }
                                }
                            }
                        }
                    } else if let Stmt::Export { path } = item {
                        let imported_mod_name = path.clone();
                        let mut to_reexport = Vec::new();
                        if let Some(imported_exports) = self.exports.get(&imported_mod_name) {
                            for (sym, export) in imported_exports {
                                if export.visibility == Visibility::Private {
                                    continue;
                                }

                                if !local_declarations.contains_key(sym) {
                                    local_scope.insert(sym.clone(), export.clone());
                                    to_reexport.push((sym.clone(), export.clone()));
                                }
                            }
                        }
                        // Apply re-exports
                        if let Some(mod_exports) = self.exports.get_mut(name) {
                            for (sym, export) in to_reexport {
                                mod_exports.insert(sym, export);
                            }
                        }
                    }
                }

                for item in body.iter_mut() {
                    self.resolve_stmt(item, &local_scope, &mod_aliases)?;
                }
            }
        }

        Ok(ast)
    }

    fn resolve_stmt(
        &self,
        stmt: &mut Stmt,
        scope: &HashMap<ustr::Ustr, ModuleExport>,
        aliases: &HashMap<ustr::Ustr, String>,
    ) -> Result<()> {
        match stmt {
            Stmt::Expr(expr) => self.resolve_expr(expr, scope, aliases)?,
            Stmt::VarDecl {
                initializer,
                type_annotation,
                ..
            } => {
                if let Some(expr) = initializer {
                    self.resolve_expr(expr, scope, aliases)?;
                }
                if let Some(ty) = type_annotation {
                    self.resolve_type(ty, scope, aliases)?;
                }
            }
            Stmt::Return(expr) => {
                if let Some(e) = expr {
                    self.resolve_expr(e, scope, aliases)?;
                }
            }
            Stmt::FuncDecl {
                body,
                params,
                return_type,
                ..
            } => {
                for p in params {
                    self.resolve_type(&mut p.type_annotation, scope, aliases)?;
                }
                if let Some(ty) = return_type {
                    self.resolve_type(ty, scope, aliases)?;
                }
                for s in body {
                    self.resolve_stmt(s, scope, aliases)?;
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
                    self.resolve_stmt(f, scope, aliases)?;
                }
                for m in methods {
                    self.resolve_stmt(m, scope, aliases)?;
                }
                if let Some(imp) = implements {
                    self.resolve_type(imp, scope, aliases)?;
                }
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.resolve_expr(condition, scope, aliases)?;
                self.resolve_stmt(then_branch, scope, aliases)?;
                if let Some(eb) = else_branch {
                    self.resolve_stmt(eb, scope, aliases)?;
                }
            }
            Stmt::While { condition, body } => {
                self.resolve_expr(condition, scope, aliases)?;
                self.resolve_stmt(body, scope, aliases)?;
            }
            Stmt::Block(stmts) => {
                for s in stmts {
                    self.resolve_stmt(s, scope, aliases)?;
                }
            }
            Stmt::Match { expr, arms } => {
                self.resolve_expr(expr, scope, aliases)?;
                for (pattern, body) in arms {
                    self.resolve_pattern(pattern, scope, aliases)?;
                    self.resolve_stmt(body, scope, aliases)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn resolve_expr(
        &self,
        expr: &mut Expr,
        scope: &HashMap<ustr::Ustr, ModuleExport>,
        aliases: &HashMap<ustr::Ustr, String>,
    ) -> Result<()> {
        match expr {
            Expr::Call { callee, args } => {
                // If callee is MemberAccess(alias, symbol), resolve it here
                if let Expr::MemberAccess {
                    object, property, ..
                } = &**callee
                {
                    if let Expr::Identifier(obj_name, _) = &**object {
                        if let Some(mod_name) = aliases.get(obj_name) {
                            if let Some(mod_exports) = self.exports.get(&ustr::Ustr::from(mod_name)) {
                                if let Some(export) = mod_exports.get(property) {
                                    if export.visibility == Visibility::Private {
                                        return Err(Report::new(ResolutionError::PrivateSymbol {
                                            name: property.to_string(),
                                            span: (0, 0),
                                        }));
                                    }
                                    *callee =
                                        Box::new(Expr::Identifier(export.mangled_name.clone().into(), pace_ast::Span::default()));
                                }
                            }
                        }
                    }
                }

                // If callee is just an Identifier, look it up in scope
                if let Expr::Identifier(name, _) = &**callee {
                    if let Some(export) = scope.get(&ustr::Ustr::from(name)) {
                        if export.mangled_name == "COLLISION" {
                            return Err(Report::new(ResolutionError::Collision {
                                name: name.to_string(),
                                span: (0, 0),
                            }));
                        }
                        *callee = Box::new(Expr::Identifier(export.mangled_name.clone().into(), pace_ast::Span::default()));
                    }
                }

                self.resolve_expr(callee, scope, aliases)?;
                for arg in args {
                    self.resolve_expr(arg, scope, aliases)?;
                }
            }
            Expr::Identifier(name, _) => {
                if name == "StringUtil" {}
                if let Some(export) = scope.get(&ustr::Ustr::from(name)) {
                    if export.mangled_name == "COLLISION" {
                        return Err(Report::new(ResolutionError::Collision {
                            name: name.to_string(),
                            span: (0, 0),
                        }));
                    }
                    *name = export.mangled_name.clone().into();
                }
            }
            Expr::Binary { left, right, .. } => {
                self.resolve_expr(left, scope, aliases)?;
                self.resolve_expr(right, scope, aliases)?;
            }
            Expr::Assign { target, value } => {
                self.resolve_expr(target, scope, aliases)?;
                self.resolve_expr(value, scope, aliases)?;
            }
            Expr::MemberAccess { object, .. } => {
                self.resolve_expr(object, scope, aliases)?;
            }
            Expr::OptionalMemberAccess { object, .. } => {
                self.resolve_expr(object, scope, aliases)?;
            }
            Expr::GenericInstantiation { callee, .. } => {
                self.resolve_expr(callee, scope, aliases)?;
            }
            Expr::InterpolatedString(exprs) => {
                for e in exprs {
                    self.resolve_expr(e, scope, aliases)?;
                }
            }
            Expr::Unwrap(inner) | Expr::Try(inner) | Expr::Await(inner) => {
                self.resolve_expr(inner, scope, aliases)?;
            }
            Expr::NullCoalesce { left, right } => {
                self.resolve_expr(left, scope, aliases)?;
                self.resolve_expr(right, scope, aliases)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn resolve_pattern(
        &self,
        pat: &mut pace_ast::Pattern,
        scope: &HashMap<ustr::Ustr, ModuleExport>,
        aliases: &HashMap<ustr::Ustr, String>,
    ) -> Result<()> {
        match pat {
            pace_ast::Pattern::Literal(expr) => self.resolve_expr(expr, scope, aliases)?,
            pace_ast::Pattern::Variant {
                enum_name, fields, ..
            } => {
                if let Some(name) = enum_name {
                    if let Some(export) = scope.get(&ustr::Ustr::from(name)) {
                        *name = export.mangled_name.clone().into();
                    }
                }
                if let Some(flds) = fields {
                    for f in flds {
                        self.resolve_pattern(f, scope, aliases)?;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn resolve_type(
        &self,
        ty: &mut pace_ast::TypeAnnotation,
        scope: &HashMap<ustr::Ustr, ModuleExport>,
        aliases: &HashMap<ustr::Ustr, String>,
    ) -> Result<()> {
        if let Some(prefix) = &ty.module_prefix {
            if let Some(mod_name) = aliases.get(prefix) {
                if let Some(mod_exports) = self.exports.get(&ustr::Ustr::from(mod_name)) {
                    if let Some(export) = mod_exports.get(&ty.name) {
                        if export.visibility == Visibility::Private {
                            return Err(Report::new(ResolutionError::PrivateSymbol {
                                name: ty.name.to_string(),
                                span: (0, 0),
                            }));
                        }
                        ty.name = ustr::Ustr::from(&export.mangled_name);
                    } else {
                        return Err(Report::new(ResolutionError::UnresolvedSymbol {
                            name: ty.name.to_string(),
                            span: (0, 0),
                        }));
                    }
                }
            } else {
                return Err(Report::new(ResolutionError::UnresolvedSymbol {
                    name: ustr::Ustr::from(&prefix).to_string(),
                    span: (0, 0),
                }));
            }
        } else if let Some(export) = scope.get(&ty.name) {
            if export.mangled_name == "COLLISION" {
                return Err(Report::new(ResolutionError::Collision {
                    name: ty.name.to_string(),
                    span: (0, 0),
                }));
            }
            ty.name = ustr::Ustr::from(&export.mangled_name);
        }
        for arg in &mut ty.args {
            self.resolve_type(arg, scope, aliases)?;
        }
        Ok(())
    }
}
