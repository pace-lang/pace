use std::collections::HashMap;
use crate::package_id::PackageId;
use crate::manifest::Manifest;

use std::path::PathBuf;

#[derive(Debug)]
pub struct PackageGraph {
    next_id: u32,
    pub manifests: HashMap<PackageId, Manifest>,
    pub paths: HashMap<PackageId, PathBuf>,
    pub dependencies: HashMap<PackageId, HashMap<String, PackageId>>,
}

impl PackageGraph {
    pub fn new() -> Self {
        Self {
            next_id: 0,
            manifests: HashMap::new(),
            paths: HashMap::new(),
            dependencies: HashMap::new(),
        }
    }

    pub fn next_id(&mut self) -> PackageId {
        let id = PackageId(self.next_id);
        self.next_id += 1;
        id
    }

    pub fn add_package(&mut self, id: PackageId, manifest: Manifest, path: PathBuf) {
        self.manifests.insert(id, manifest);
        self.paths.insert(id, path);
        self.dependencies.insert(id, HashMap::new());
    }

    pub fn add_dependency(&mut self, package: PackageId, alias: String, dep: PackageId) {
        if let Some(deps) = self.dependencies.get_mut(&package) {
            deps.insert(alias, dep);
        }
    }
}
