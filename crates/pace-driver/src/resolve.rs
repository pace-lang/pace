use pace_ast::{Stmt, Expr, Visibility};
use std::collections::HashMap;
use miette::{Result, Diagnostic, Report};
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
    pub name: String,
    pub visibility: Visibility,
    pub mangled_name: String,
}

pub struct SymbolResolver {
    // module_name -> (symbol_name -> export)
    pub exports: HashMap<String, HashMap<String, ModuleExport>>,
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
                        Stmt::FuncDecl { name: n, visibility, .. } => {
                            is_export = true;
                            vis = visibility.clone();
                            original_name = n.clone();
                        }
                        Stmt::ClassDecl { name: n, .. } |
                        Stmt::StructDecl { name: n, .. } |
                        Stmt::EnumDecl { name: n, .. } |
                        Stmt::InterfaceDecl { name: n, .. } => {
                            is_export = true;
                            original_name = n.clone();
                        }
                        _ => {}
                    }
                    
                    if is_export {
                        let mangled_name = if name.starts_with("pkg:") && !name.starts_with("pkg:std:") {
                            format!("{}__{}", name.replace("pkg:", "").replace("-", "_"), original_name)
                        } else {
                            original_name.clone()
                        };

                        module_exports.insert(original_name.clone(), ModuleExport {
                            name: original_name.clone(),
                            visibility: vis,
                            mangled_name: mangled_name.clone(),
                        });

                        match item {
                            Stmt::FuncDecl { name: n, .. } => *n = mangled_name,
                            Stmt::ClassDecl { name: n, .. } => *n = mangled_name,
                            Stmt::StructDecl { name: n, .. } => *n = mangled_name,
                            Stmt::EnumDecl { name: n, .. } => *n = mangled_name,
                            Stmt::InterfaceDecl { name: n, .. } => *n = mangled_name,
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
                let mut local_scope: HashMap<String, ModuleExport> = HashMap::new();
                let mut mod_aliases: HashMap<String, String> = HashMap::new();
                
                if let Some(exports) = self.exports.get(name) {
                    for (k, v) in exports {
                        local_scope.insert(k.clone(), v.clone());
                    }
                }

                for item in body.iter() {
                    if let Stmt::Import { path, alias, show, hide } = item {
                        let imported_mod_name = if path.starts_with("./") || path.starts_with("../") {
                            std::path::Path::new(path).file_stem().unwrap().to_string_lossy().to_string()
                        } else {
                            format!("pkg:{}", path)
                        };
                        if let Some(imported_exports) = self.exports.get(&imported_mod_name) {
                            if let Some(alias_name) = alias {
                                mod_aliases.insert(alias_name.clone(), imported_mod_name.clone());
                            } else {
                                for (sym, export) in imported_exports {
                                    if export.visibility == Visibility::Private { continue; }
                                    if let Some(hide_list) = hide {
                                        if hide_list.contains(sym) { continue; }
                                    }
                                    if let Some(show_list) = show {
                                        if !show_list.contains(sym) { continue; }
                                    }
                                    
                                    if let Some(existing) = local_scope.get(sym) {
                                        if existing.mangled_name != export.mangled_name {
                                            local_scope.insert(sym.clone(), ModuleExport {
                                                name: sym.clone(),
                                                visibility: Visibility::Public,
                                                mangled_name: "COLLISION".to_string(),
                                            });
                                        }
                                    } else {
                                        local_scope.insert(sym.clone(), export.clone());
                                    }
                                }
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

    fn resolve_stmt(&self, stmt: &mut Stmt, scope: &HashMap<String, ModuleExport>, aliases: &HashMap<String, String>) -> Result<()> {
        match stmt {
            Stmt::Expr(expr) => self.resolve_expr(expr, scope, aliases)?,
            Stmt::VarDecl { initializer, type_annotation, .. } => {
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
            Stmt::FuncDecl { body, params, return_type, .. } => {
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
            Stmt::ClassDecl { fields, methods, implements, .. } => {
                for f in fields { self.resolve_stmt(f, scope, aliases)?; }
                for m in methods { self.resolve_stmt(m, scope, aliases)?; }
                if let Some(imp) = implements { self.resolve_type(imp, scope, aliases)?; }
            }
            Stmt::If { condition, then_branch, else_branch } => {
                self.resolve_expr(condition, scope, aliases)?;
                self.resolve_stmt(then_branch, scope, aliases)?;
                if let Some(eb) = else_branch { self.resolve_stmt(eb, scope, aliases)?; }
            }
            Stmt::While { condition, body } => {
                self.resolve_expr(condition, scope, aliases)?;
                self.resolve_stmt(body, scope, aliases)?;
            }
            Stmt::Block(stmts) => {
                for s in stmts { self.resolve_stmt(s, scope, aliases)?; }
            }
            _ => {}
        }
        Ok(())
    }

    fn resolve_expr(&self, expr: &mut Expr, scope: &HashMap<String, ModuleExport>, aliases: &HashMap<String, String>) -> Result<()> {
        match expr {
            Expr::Call { callee, args } => {
                // If callee is MemberAccess(alias, symbol), resolve it here
                if let Expr::MemberAccess { object, property, .. } = &**callee {
                    if let Expr::Identifier(obj_name) = &**object {
                        if let Some(mod_name) = aliases.get(obj_name) {
                            if let Some(mod_exports) = self.exports.get(mod_name) {
                                if let Some(export) = mod_exports.get(property) {
                                    if export.visibility == Visibility::Private {
                                        return Err(Report::new(ResolutionError::PrivateSymbol { name: property.clone(), span: (0, 0) }));
                                    }
                                    *callee = Box::new(Expr::Identifier(export.mangled_name.clone()));
                                }
                            }
                        }
                    }
                }
                
                // If callee is just an Identifier, look it up in scope
                if let Expr::Identifier(name) = &**callee {
                    if let Some(export) = scope.get(name) {
                        if export.mangled_name == "COLLISION" {
                            return Err(Report::new(ResolutionError::Collision { name: name.clone(), span: (0, 0) }));
                        }
                        *callee = Box::new(Expr::Identifier(export.mangled_name.clone()));
                    }
                }

                self.resolve_expr(callee, scope, aliases)?;
                for arg in args {
                    self.resolve_expr(arg, scope, aliases)?;
                }
            }
            Expr::Identifier(name) => {
                if let Some(export) = scope.get(name) {
                    if export.mangled_name == "COLLISION" {
                        return Err(Report::new(ResolutionError::Collision { name: name.clone(), span: (0, 0) }));
                    }
                    *name = export.mangled_name.clone();
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
            _ => {}
        }
        Ok(())
    }

    fn resolve_type(&self, ty: &mut pace_ast::TypeAnnotation, scope: &HashMap<String, ModuleExport>, aliases: &HashMap<String, String>) -> Result<()> {
        if let Some(export) = scope.get(&ty.name) {
            if export.mangled_name == "COLLISION" {
                return Err(Report::new(ResolutionError::Collision { name: ty.name.clone(), span: (0, 0) }));
            }
            ty.name = export.mangled_name.clone();
        } else {
            // Might be something like `lib.Struct`
            // But TypeAnnotation just has `name`. We'd need to parse dot in TypeAnnotation if we supported it.
            // For now we just resolve simple types.
        }
        for arg in &mut ty.args {
            self.resolve_type(arg, scope, aliases)?;
        }
        Ok(())
    }
}
