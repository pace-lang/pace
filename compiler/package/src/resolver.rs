use std::path::{Path, PathBuf};
use crate::manifest::Dependency;
use std::env;

pub trait DependencySource {
    fn resolve(&self, name: &str, dep: &Dependency, base_path: &Path) -> Option<PathBuf>;
}

pub struct PathSource;

impl DependencySource for PathSource {
    fn resolve(&self, _name: &str, dep: &Dependency, base_path: &Path) -> Option<PathBuf> {
        if let Dependency::Path { path } = dep {
            let mut resolved = base_path.to_path_buf();
            resolved.push(path);
            return Some(resolved.canonicalize().unwrap_or(resolved));
        }
        None
    }
}

pub struct RegistrySource;

impl DependencySource for RegistrySource {
    fn resolve(&self, name: &str, dep: &Dependency, _base_path: &Path) -> Option<PathBuf> {
        if let Dependency::Version(version) = dep {
            let home = env::var("HOME").unwrap_or_else(|_| String::from("~"));
            let mut path = PathBuf::from(home);
            path.push(".pace");
            path.push("packages");
            path.push(format!("{}-{}", name, version));
            return Some(path);
        }
        None
    }
}

pub struct DependencyResolver {
    sources: Vec<Box<dyn DependencySource>>,
}

impl DependencyResolver {
    pub fn new() -> Self {
        Self {
            sources: vec![
                Box::new(PathSource),
                Box::new(RegistrySource),
            ],
        }
    }

    pub fn resolve(&self, name: &str, dep: &Dependency, base_path: &Path) -> Option<PathBuf> {
        for source in &self.sources {
            if let Some(path) = source.resolve(name, dep, base_path) {
                return Some(path);
            }
        }
        None
    }
}
