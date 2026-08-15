use std::path::{Path, PathBuf};
use std::fs;
use crate::graph::ModuleGraph;
use crate::module::Module;
use crate::module_id::ModuleId;
use ast::StmtKind;
use diagnostics::Diagnostic;

use package::graph::PackageGraph;
use package::package_id::PackageId;

pub struct ModuleLoader<'a> {
    graph: ModuleGraph<'a>,
    loaded_paths: std::collections::HashMap<PathBuf, ModuleId>,
    pub errors: Vec<Diagnostic>,
    package_graph: Option<&'a PackageGraph>,
    pub source_map: diagnostics::SourceMap,
    pub session: &'a session::CompilerSession,
}

impl<'a> ModuleLoader<'a> {
    pub fn new(package_graph: Option<&'a PackageGraph>, session: &'a session::CompilerSession) -> Self {
        Self {
            graph: ModuleGraph::new(),
            loaded_paths: std::collections::HashMap::new(),
            errors: Vec::new(),
            package_graph,
            source_map: diagnostics::SourceMap::new(),
            session,
        }
    }

    pub fn load_root(&mut self, root_path: &Path) -> Option<ModuleId> {
        let abs_path = root_path.canonicalize().unwrap_or_else(|_| root_path.to_path_buf());
        self.load_file(&abs_path)
    }

    fn load_file(&mut self, path: &Path) -> Option<ModuleId> {
        if let Some(&id) = self.loaded_paths.get(path) {
            return Some(id);
        }

        let source = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(e) => {
                // We don't have span info for file open errors easily, just pushing a dummy diagnostic for now.
                // In reality, we'd log this better.
                self.errors.push(diagnostics::DiagnosticBuilder::error(
                    diagnostics::DiagnosticCode::InvalidToken, 
                    format!("Failed to read file {:?}: {}", path, e), 
                    diagnostics::Span::new(0, 0, 0, diagnostics::Location::new(0, 0), diagnostics::Location::new(0, 0))
                ).build());
                return None;
            }
        };

        let file_id = self.source_map.add_file(path.to_path_buf(), source.clone());

        let mut scanner = lexer::Scanner::new(file_id, &source);
        let tokens = scanner.scan_tokens(self.session);
        if !scanner.diagnostics.is_empty() {
            self.errors.extend(scanner.diagnostics);
            return None;
        }

        let mut parser = parser::Parser::new(tokens, self.session);
        let (ast, parse_errors) = parser.parse();
        if !parse_errors.is_empty() {
            self.errors.extend(parse_errors);
            return None;
        }

        let mut final_ast = ast;
        
        // Inject synthetic Prelude imports into every module (except std itself) to implicitly provide core types.
        let mut is_std = false;
        let mut std_root_path = None;
        if let Some(pg) = self.package_graph {
            for (id, pkg_path) in &pg.paths {
                if let Some(manifest) = pg.manifests.get(id)
                    && manifest.package.name == "std" {
                        std_root_path = Some(pkg_path.clone());
                    }
                if path.starts_with(pkg_path)
                    && let Some(manifest) = pg.manifests.get(id)
                        && manifest.package.name == "std" {
                            is_std = true;
                        }
            }
        }
        
        if !is_std {
            let mut prelude_imports = Vec::new();
            if let Some(std_root) = std_root_path {
                let std_src = std_root.join("src");
                let mut stack = vec![std_src.clone()];
                while let Some(dir) = stack.pop() {
                    if let Ok(entries) = std::fs::read_dir(&dir) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            if path.is_dir() {
                                stack.push(path);
                            } else if path.extension().is_some_and(|ext| ext == "pace")
                                && let Ok(rel_path) = path.strip_prefix(&std_src) {
                                    let mut import_path = "std".to_string();
                                    for comp in rel_path.components() {
                                        if let std::path::Component::Normal(name) = comp {
                                            let name_str = name.to_string_lossy();
                                            if let Some(stripped) = name_str.strip_suffix(".pace") {
                                                import_path.push('/');
                                                import_path.push_str(stripped);
                                            } else {
                                                import_path.push('/');
                                                import_path.push_str(&name_str);
                                            }
                                        }
                                    }
                                    prelude_imports.push(ast::Stmt::new(ast::StmtKind::Import {
                                        path: self.session.interner.borrow_mut().intern(&format!("\"{}\"", import_path)),
                                        alias: None,
                                        show: vec![],
                                        hide: vec![],
                                    }, diagnostics::Span::new(0, 0, 0, diagnostics::Location::new(0, 0), diagnostics::Location::new(0, 0))));
                                }
                        }
                    }
                }
            }
            
            let mut temp = prelude_imports;
            temp.extend(final_ast);
            final_ast = temp;
        }

        let module_id = self.graph.next_id();
        self.loaded_paths.insert(path.to_path_buf(), module_id);

        // Find imports and recursively load
        let mut dependencies = Vec::new();
        for stmt in &final_ast {
            if let StmtKind::Import { path: import_path_sym, .. } = &stmt.kind {
                let import_path_str = self.session.interner.borrow().lookup(*import_path_sym).to_string();
                let clean_path = import_path_str.trim_matches('"').trim_matches('\'').to_string();

                let resolved_import_path;

                let mut local_resolved = path.parent().unwrap_or_else(|| Path::new("")).to_path_buf();
                local_resolved.push(&clean_path);
                if !clean_path.ends_with(".pace") {
                    local_resolved.set_extension("pace");
                }

                if local_resolved.exists() {
                    resolved_import_path = Some(local_resolved.canonicalize().unwrap_or(local_resolved));
                } else if clean_path.starts_with("./") || clean_path.starts_with("../") {
                    self.errors.push(diagnostics::DiagnosticBuilder::error(
                        diagnostics::DiagnosticCode::InvalidToken,
                        format!("Could not find local file '{}'.", local_resolved.display()),
                        stmt.span
                    ).build());
                    continue;
                } else {
                    // Package import
                    let parts: Vec<&str> = clean_path.split('/').collect();
                    let pkg_name = parts[0];
                    let mut resolved = None;
                    
                    if let Some(pg) = self.package_graph {
                        // Find which package owns this file. For simplicity in Phase 3, 
                        // we just look through all packages since we don't have a mapping of file -> PackageId yet.
                        // Ideally we find the PackageId of `path`, then look at its dependencies.
                        // Let's assume the root package (ID 0) for now if we can't find it.
                        let mut owner_pkg = PackageId(0);
                        for (id, pkg_path) in &pg.paths {
                            if path.starts_with(pkg_path) {
                                owner_pkg = *id;
                                break;
                            }
                        }
                        if let Some(deps) = pg.dependencies.get(&owner_pkg)
                            && let Some(dep_pkg_id) = deps.get(pkg_name)
                                && let Some(dep_pkg_path) = pg.paths.get(dep_pkg_id) {
                                    let mut p = dep_pkg_path.clone();
                                    p.push("src");
                                    for part in parts.iter().skip(1) {
                                        p.push(part);
                                    }
                                    if parts.len() == 1 {
                                        // default to lib.pace or main.pace if only package name is provided
                                    }
                                    p.set_extension("pace");
                                    resolved = Some(p.canonicalize().unwrap_or(p));
                                }
                    }
                    
                    if let Some(r) = resolved {
                        resolved_import_path = Some(r);
                    } else {
                        self.errors.push(diagnostics::DiagnosticBuilder::error(
                            diagnostics::DiagnosticCode::InvalidToken, 
                            format!("Could not resolve package import '{}'.", clean_path), 
                            stmt.span
                        ).build());
                        continue;
                    }
                }
                
                if let Some(abs_path) = resolved_import_path
                    && let Some(dep_id) = self.load_file(&abs_path) {
                        dependencies.push((clean_path.to_string(), dep_id));
                    }
            }
        }

        self.graph.add_module(Module::new(module_id, path.to_path_buf(), final_ast));
        
        for (import_str, dep) in dependencies {
            self.graph.add_dependency(module_id, dep);
            self.graph.add_import_mapping(module_id, import_str, dep);
        }

        Some(module_id)
    }

    pub fn into_graph(self) -> (ModuleGraph<'a>, diagnostics::SourceMap) {
        (self.graph, self.source_map)
    }
}
