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

impl Default for SymbolResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl SymbolResolver {
    pub fn new() -> Self {
        Self {
            exports: HashMap::new(),
        }
    }

    pub fn run(
        arena: &mut pace_ast::arena::AstArena,
        ast: Vec<pace_ast::arena::StmtId>,
    ) -> Result<Vec<pace_ast::arena::StmtId>> {
        let mut resolver = Self::new();
        resolver.resolve(arena, ast)
    }

    pub fn resolve(
        &mut self,
        arena: &mut pace_ast::arena::AstArena,
        ast: Vec<pace_ast::arena::StmtId>,
    ) -> Result<Vec<pace_ast::arena::StmtId>> {
        // Pass 1: Collect exports for all modules and mangle their definitions
        for &stmt_id in &ast {
            let stmt = arena.get_stmt(stmt_id).clone();
            if let Stmt::Module { name, ref body } = stmt {
                let mut module_exports = HashMap::new();
                for &item_id in body.iter() {
                    let mut item_stmt = arena.get_stmt(item_id).clone();
                    let item = &mut item_stmt;
                    let mut is_export = false;
                    let mut vis = Visibility::Public;
                    let mut original_name = String::new();
                    let mut item_is_extern = false;

                    match item {
                        Stmt::FuncDecl {
                            name: n,
                            visibility,
                            is_extern,
                            ..
                        } => {
                            is_export = true;
                            vis = visibility.clone();
                            original_name = n.to_string();
                            item_is_extern = *is_extern;
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
                        } else if original_name == "main" || original_name == "StringBuilder" || item_is_extern {
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
                    *arena.get_stmt_mut(item_id) = item_stmt;
                }
                self.exports.insert(name, module_exports);
            }
            *arena.get_stmt_mut(stmt_id) = stmt;
        }

        // Pass 2: Resolve references within each module
        for &stmt_id in &ast {
            let stmt = arena.get_stmt(stmt_id).clone();
            if let Stmt::Module { name, ref body } = stmt {
                let mut local_scope: HashMap<ustr::Ustr, ModuleExport> = HashMap::new();
                let mut mod_aliases: HashMap<ustr::Ustr, String> = HashMap::new();

                let mut local_declarations: HashMap<ustr::Ustr, ModuleExport> = HashMap::new();
                if let Some(exports) = self.exports.get(&name) {
                    for (k, v) in exports {
                        local_declarations.insert(*k, v.clone());
                        local_scope.insert(*k, v.clone());
                    }
                }

                for &item_id in body.iter() {
                    let item = arena.get_stmt(item_id);
                    if let Stmt::Import {
                        path,
                        alias,
                        show,
                        hide,
                    } = item
                    {
                        let imported_mod_name = *path;
                        if let Some(imported_exports) = self.exports.get(&imported_mod_name) {
                            if let Some(alias_name) = alias {
                                mod_aliases.insert(*alias_name, imported_mod_name.to_string());
                            } else {
                                for (sym, export) in imported_exports {
                                    if export.visibility == Visibility::Private {
                                        continue;
                                    }
                                    if let Some(hide_list) = hide
                                        && hide_list.contains(sym)
                                    {
                                        continue;
                                    }
                                    if let Some(show_list) = show
                                        && !show_list.contains(sym)
                                    {
                                        continue;
                                    }

                                    if local_declarations.contains_key(sym) {
                                        continue; // Local declarations shadow imports implicitly
                                    }

                                    if let Some(existing) = local_scope.get(sym) {
                                        if existing.mangled_name != export.mangled_name {
                                            local_scope.insert(
                                                *sym,
                                                ModuleExport {
                                                    name: *sym,
                                                    visibility: Visibility::Public,
                                                    mangled_name: "COLLISION".to_string(),
                                                },
                                            );
                                        }
                                    } else {
                                        local_scope.insert(*sym, export.clone());
                                    }
                                }
                            }
                        }
                    } else if let Stmt::Export { path } = item {
                        let imported_mod_name = *path;
                        let mut to_reexport = Vec::new();
                        if let Some(imported_exports) = self.exports.get(&imported_mod_name) {
                            for (sym, export) in imported_exports {
                                if export.visibility == Visibility::Private {
                                    continue;
                                }

                                if !local_declarations.contains_key(sym) {
                                    local_scope.insert(*sym, export.clone());
                                    to_reexport.push((*sym, export.clone()));
                                }
                            }
                        }
                        // Apply re-exports
                        if let Some(mod_exports) = self.exports.get_mut(&name) {
                            for (sym, export) in to_reexport {
                                mod_exports.insert(sym, export);
                            }
                        }
                    }
                }

                for &item_id in body.iter() {
                    self.resolve_stmt(arena, item_id, &local_scope, &mod_aliases)?;
                }
            }
            *arena.get_stmt_mut(stmt_id) = stmt;
        }

        Ok(ast)
    }

    fn resolve_stmt(
        &self,
        arena: &mut pace_ast::arena::AstArena,
        stmt_id: pace_ast::arena::StmtId,
        scope: &HashMap<ustr::Ustr, ModuleExport>,
        aliases: &HashMap<ustr::Ustr, String>,
    ) -> Result<()> {
        let mut stmt = arena.get_stmt(stmt_id).clone();
        match &mut stmt {
            Stmt::Expr(expr) => self.resolve_expr(arena, *expr, scope, aliases)?,
            Stmt::VarDecl {
                initializer,
                type_annotation,
                ..
            } => {
                if let Some(expr) = initializer {
                    self.resolve_expr(arena, *expr, scope, aliases)?;
                }
                if let Some(ty) = type_annotation {
                    self.resolve_type(ty, scope, aliases)?;
                }
            }
            Stmt::Return(expr) => {
                if let Some(e) = expr {
                    self.resolve_expr(arena, *e, scope, aliases)?;
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
                    self.resolve_stmt(arena, *s, scope, aliases)?;
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
                    self.resolve_stmt(arena, *f, scope, aliases)?;
                }
                for m in methods {
                    self.resolve_stmt(arena, *m, scope, aliases)?;
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
                self.resolve_expr(arena, *condition, scope, aliases)?;
                self.resolve_stmt(arena, *then_branch, scope, aliases)?;
                if let Some(eb) = else_branch {
                    self.resolve_stmt(arena, *eb, scope, aliases)?;
                }
            }
            Stmt::While { condition, body } => {
                self.resolve_expr(arena, *condition, scope, aliases)?;
                self.resolve_stmt(arena, *body, scope, aliases)?;
            }
            Stmt::Block(stmts) => {
                for s in stmts {
                    self.resolve_stmt(arena, *s, scope, aliases)?;
                }
            }
            Stmt::Match { expr, arms } => {
                self.resolve_expr(arena, *expr, scope, aliases)?;
                for (pattern, body) in arms {
                    self.resolve_pattern(arena, pattern, scope, aliases)?;
                    self.resolve_stmt(arena, *body, scope, aliases)?;
                }
            }
            Stmt::InterfaceDecl { methods, .. } => {
                for m in methods {
                    self.resolve_stmt(arena, *m, scope, aliases)?;
                }
            }
            Stmt::StructDecl { fields, .. } => {
                for f in fields {
                    self.resolve_stmt(arena, *f, scope, aliases)?;
                }
            }
            Stmt::EnumDecl { variants, .. } => {
                for v in variants {
                    if let Some(flds) = &mut v.fields {
                        for f in flds {
                            self.resolve_type(f, scope, aliases)?;
                        }
                    }
                }
            }
            _ => {}
        }
        *arena.get_stmt_mut(stmt_id) = stmt;
        Ok(())
    }

    fn resolve_expr(
        &self,
        arena: &mut pace_ast::arena::AstArena,
        expr_id: pace_ast::arena::ExprId,
        scope: &HashMap<ustr::Ustr, ModuleExport>,
        aliases: &HashMap<ustr::Ustr, String>,
    ) -> Result<()> {
        let mut expr = arena.get_expr(expr_id).clone();
        match &mut expr {
            Expr::Call { callee, args } => {
                // If callee is MemberAccess(alias, symbol), resolve it here
                if let Expr::MemberAccess {
                    object, property, ..
                } = arena.get_expr(*callee)
                    && let Expr::Identifier(obj_name, _) = arena.get_expr(*object)
                    && let Some(mod_name) = aliases.get(obj_name)
                    && let Some(mod_exports) =
                        self.exports.get(&ustr::Ustr::from(mod_name.as_str()))
                    && let Some(export) = mod_exports.get(property)
                {
                    if export.visibility == Visibility::Private {
                        return Err(Report::new(ResolutionError::PrivateSymbol {
                            name: property.to_string(),
                            span: (0, 0),
                        }));
                    }
                    *callee = arena.alloc_expr(Expr::Identifier(
                        export.mangled_name.clone().into(),
                        pace_ast::Span::default(),
                    ), pace_ast::Span::default());
                }

                // If callee is just an Identifier, look it up in scope
                if let Expr::Identifier(name, _) = arena.get_expr(*callee)
                    && let Some(export) = scope.get(&ustr::Ustr::from(name.as_str()))
                {
                    if export.mangled_name == "COLLISION" {
                        return Err(Report::new(ResolutionError::Collision {
                            name: name.to_string(),
                            span: (0, 0),
                        }));
                    }
                    *callee = arena.alloc_expr(Expr::Identifier(
                        export.mangled_name.clone().into(),
                        pace_ast::Span::default(),
                    ), pace_ast::Span::default());
                }

                self.resolve_expr(arena, *callee, scope, aliases)?;
                for arg in args {
                    self.resolve_expr(arena, *arg, scope, aliases)?;
                }
            }
            Expr::Identifier(name, _) => {
                if let Some(export) = scope.get(&ustr::Ustr::from(name.as_str())) {
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
                self.resolve_expr(arena, *left, scope, aliases)?;
                self.resolve_expr(arena, *right, scope, aliases)?;
            }
            Expr::Assign { target, value } => {
                self.resolve_expr(arena, *target, scope, aliases)?;
                self.resolve_expr(arena, *value, scope, aliases)?;
            }
            Expr::MemberAccess { object, .. } => {
                self.resolve_expr(arena, *object, scope, aliases)?;
            }
            Expr::OptionalMemberAccess { object, .. } => {
                self.resolve_expr(arena, *object, scope, aliases)?;
            }
            Expr::GenericInstantiation { callee, .. } => {
                self.resolve_expr(arena, *callee, scope, aliases)?;
            }
            Expr::InterpolatedString(exprs) => {
                for e in exprs {
                    self.resolve_expr(arena, *e, scope, aliases)?;
                }
            }
            Expr::Unwrap(inner) | Expr::Try(inner) | Expr::Await(inner) => {
                self.resolve_expr(arena, *inner, scope, aliases)?;
            }
            Expr::NullCoalesce { left, right } => {
                self.resolve_expr(arena, *left, scope, aliases)?;
                self.resolve_expr(arena, *right, scope, aliases)?;
            }
            _ => {}
        }
        *arena.get_expr_mut(expr_id) = expr;
        Ok(())
    }

    fn resolve_pattern(
        &self,
        arena: &mut pace_ast::arena::AstArena,
        pat: &mut pace_ast::Pattern,
        scope: &HashMap<ustr::Ustr, ModuleExport>,
        aliases: &HashMap<ustr::Ustr, String>,
    ) -> Result<()> {
        match pat {
            pace_ast::Pattern::Literal(expr) => self.resolve_expr(arena, *expr, scope, aliases)?,
            pace_ast::Pattern::Variant {
                enum_name, fields, ..
            } => {
                if let Some(name) = enum_name
                    && let Some(export) = scope.get(&ustr::Ustr::from(name.as_str()))
                {
                    *name = export.mangled_name.clone().into();
                }
                if let Some(flds) = fields {
                    for f in flds {
                        self.resolve_pattern(arena, f, scope, aliases)?;
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
                if let Some(mod_exports) = self.exports.get(&ustr::Ustr::from(mod_name.as_str())) {
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
                    name: ustr::Ustr::from(prefix).to_string(),
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
