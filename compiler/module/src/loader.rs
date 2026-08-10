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
    graph: ModuleGraph,
    loaded_paths: std::collections::HashMap<PathBuf, ModuleId>,
    pub errors: Vec<Diagnostic>,
    package_graph: Option<&'a PackageGraph>,
}

impl<'a> ModuleLoader<'a> {
    pub fn new(package_graph: Option<&'a PackageGraph>) -> Self {
        Self {
            graph: ModuleGraph::new(),
            loaded_paths: std::collections::HashMap::new(),
            errors: Vec::new(),
            package_graph,
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
                    &format!("Failed to read file {:?}: {}", path, e), 
                    diagnostics::Span::new(0, 0, diagnostics::Location::new(0, 0), diagnostics::Location::new(0, 0))
                ).build());
                return None;
            }
        };

        let mut scanner = lexer::Scanner::new(&source);
        let tokens = scanner.scan_tokens();
        if !scanner.diagnostics.is_empty() {
            self.errors.extend(scanner.diagnostics);
            return None;
        }

        let mut parser = parser::Parser::new(tokens);
        let (ast, parse_errors) = parser.parse();
        if !parse_errors.is_empty() {
            self.errors.extend(parse_errors);
            return None;
        }

        let module_id = self.graph.next_id();
        self.loaded_paths.insert(path.to_path_buf(), module_id);

        // Find imports and recursively load
        let mut dependencies = Vec::new();
        for stmt in &ast {
            if let StmtKind::Import { path: import_path_str, .. } = &stmt.kind {
                let clean_path = import_path_str.trim_matches('"').trim_matches('\'');
                let is_local = clean_path.starts_with("./") || clean_path.starts_with("../");
                
                let abs_import_path;
                
                if is_local {
                    if !clean_path.ends_with(".pace") {
                        self.errors.push(diagnostics::DiagnosticBuilder::error(
                            diagnostics::DiagnosticCode::InvalidToken, 
                            "Local file imports must end with '.pace'.", 
                            stmt.span.clone()
                        ).build());
                        continue;
                    }
                    let mut resolved = path.parent().unwrap_or_else(|| Path::new("")).to_path_buf();
                    resolved.push(clean_path);
                    abs_import_path = Some(resolved.canonicalize().unwrap_or(resolved));
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
                        
                        if let Some(deps) = pg.dependencies.get(&owner_pkg) {
                            if let Some(dep_pkg_id) = deps.get(pkg_name) {
                                if let Some(dep_pkg_path) = pg.paths.get(dep_pkg_id) {
                                    let mut p = dep_pkg_path.clone();
                                    for part in parts.iter().skip(1) {
                                        p.push(part);
                                    }
                                    if parts.len() == 1 {
                                        // e.g. import "http" -> resolves to http's main file? 
                                        // Let's assume the name is the file name for now if len == 1? No, just add .pace.
                                    }
                                    p.set_extension("pace");
                                    resolved = Some(p.canonicalize().unwrap_or(p));
                                }
                            }
                        }
                    }
                    
                    if let Some(r) = resolved {
                        abs_import_path = Some(r);
                    } else {
                        self.errors.push(diagnostics::DiagnosticBuilder::error(
                            diagnostics::DiagnosticCode::InvalidToken, 
                            &format!("Could not resolve package import '{}'.", clean_path), 
                            stmt.span.clone()
                        ).build());
                        continue;
                    }
                }
                
                if let Some(p) = abs_import_path {
                    if let Some(dep_id) = self.load_file(&p) {
                        dependencies.push((clean_path.to_string(), dep_id));
                    }
                }
            }
        }

        let module = Module::new(module_id, path.to_path_buf(), ast);
        self.graph.add_module(module);
        
        for (import_str, dep) in dependencies {
            self.graph.add_dependency(module_id, dep);
            self.graph.add_import_mapping(module_id, import_str, dep);
        }

        Some(module_id)
    }

    pub fn into_graph(self) -> ModuleGraph {
        self.graph
    }
}
