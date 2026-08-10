use std::path::{Path, PathBuf};
use std::fs;
use crate::graph::PackageGraph;
use crate::package_id::PackageId;
use crate::manifest::Manifest;
use crate::resolver::DependencyResolver;
use diagnostics::{Diagnostic, DiagnosticBuilder, DiagnosticCode, Span, Location};

pub struct PackageManager {
    pub graph: PackageGraph,
    resolver: DependencyResolver,
    pub errors: Vec<Diagnostic>,
    loaded_paths: std::collections::HashMap<PathBuf, PackageId>,
}

impl PackageManager {
    pub fn new() -> Self {
        Self {
            graph: PackageGraph::new(),
            resolver: DependencyResolver::new(),
            errors: Vec::new(),
            loaded_paths: std::collections::HashMap::new(),
        }
    }

    pub fn load_root(&mut self, root_dir: &Path) -> Option<PackageId> {
        let abs_dir = root_dir.canonicalize().unwrap_or_else(|_| root_dir.to_path_buf());
        self.load_package(&abs_dir)
    }

    fn load_package(&mut self, dir: &Path) -> Option<PackageId> {
        if let Some(&id) = self.loaded_paths.get(dir) {
            return Some(id);
        }

        let toml_path = dir.join("pace.toml");
        let source = match fs::read_to_string(&toml_path) {
            Ok(content) => content,
            Err(e) => {
                self.error(&format!("Failed to read manifest at {:?}: {}", toml_path, e));
                return None;
            }
        };

        let manifest: Manifest = match toml::from_str(&source) {
            Ok(m) => m,
            Err(e) => {
                self.error(&format!("Failed to parse pace.toml in {:?}: {}", dir, e));
                return None;
            }
        };

        let package_id = self.graph.next_id();
        self.loaded_paths.insert(dir.to_path_buf(), package_id);
        
        let deps = manifest.dependencies.clone();
        
        // Register the package in the graph
        self.graph.add_package(package_id, manifest, dir.to_path_buf());

        // Recursively load dependencies
        for (dep_name, dep_spec) in deps {
            if let Some(dep_path) = self.resolver.resolve(&dep_name, &dep_spec, dir) {
                if let Some(dep_id) = self.load_package(&dep_path) {
                    self.graph.add_dependency(package_id, dep_name, dep_id);
                } else {
                    self.error(&format!("Failed to load dependency '{}' from {:?}", dep_name, dep_path));
                }
            } else {
                self.error(&format!("Could not resolve dependency '{}'", dep_name));
            }
        }

        Some(package_id)
    }

    fn error(&mut self, message: &str) {
        self.errors.push(DiagnosticBuilder::error(
            DiagnosticCode::InvalidToken, 
            message, 
            Span::new(0, 0, Location::new(0, 0), Location::new(0, 0))
        ).build());
    }

    pub fn into_graph(self) -> PackageGraph {
        self.graph
    }
}
