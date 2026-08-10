use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Manifest {
    pub package: PackageInfo,
    #[serde(default)]
    pub dependencies: std::collections::HashMap<String, Dependency>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum Dependency {
    Version(String),
    Path { path: String },
}
