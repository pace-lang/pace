use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use miette::{Diagnostic, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaceToml {
    pub package: Package,
    #[serde(default)]
    pub sdk: Option<HashMap<String, String>>,
    #[serde(default)]
    pub dependencies: HashMap<String, Dependency>,
    #[serde(rename = "dev-dependencies", default)]
    pub dev_dependencies: HashMap<String, Dependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub license: Option<String>,
    pub authors: Option<Vec<String>>,
    pub repository: Option<String>,
    #[serde(default)]
    pub platforms: Option<Vec<String>>,
    #[serde(default)]
    pub keywords: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Dependency {

    /// A local path dependency (e.g., path = "../foo")
    Path {
        path: String,
    },
    /// A version dependency (for a future registry)
    Version(String),
}

#[derive(Debug, thiserror::Error, Diagnostic)]
pub enum ManifestError {
    #[error("Failed to read pace.toml file: {0}")]
    #[diagnostic(code(pace_pkg::io_error))]
    IoError(#[from] std::io::Error),
    
    #[error("Failed to parse pace.toml: {0}")]
    #[diagnostic(code(pace_pkg::parse_error))]
    ParseError(#[from] toml::de::Error),
    
    #[error("Failed to serialize pace.toml: {0}")]
    #[diagnostic(code(pace_pkg::serialize_error))]
    SerializeError(#[from] toml::ser::Error),
    
    #[error("Failed to edit pace.toml: {0}")]
    #[diagnostic(code(pace_pkg::edit_error))]
    EditError(String),
}

impl PaceToml {
    /// Load and parse a pace.toml file from the given directory path
    pub fn load_from_dir(dir: &Path) -> Result<Self, ManifestError> {
        let toml_path = dir.join("pace.toml");
        let content = fs::read_to_string(&toml_path)?;
        let manifest: PaceToml = toml::from_str(&content)?;
        Ok(manifest)
    }

    /// Save the manifest to a directory
    pub fn save_to_dir(&self, dir: &Path) -> Result<(), ManifestError> {
        let toml_path = dir.join("pace.toml");
        let content = toml::to_string_pretty(self)?;
        fs::write(&toml_path, content)?;
        Ok(())
    }

    /// Add a dependency while strictly preserving user formatting and comments
    pub fn add_dependency(dir: &Path, name: &str, dep: Dependency) -> Result<(), ManifestError> {
        let toml_path = dir.join("pace.toml");
        let content = fs::read_to_string(&toml_path)?;
        let mut doc = content.parse::<toml_edit::DocumentMut>().map_err(|e| ManifestError::EditError(e.to_string()))?;
        
        let deps = doc.entry("dependencies").or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
        
        if let Some(table) = deps.as_table_mut() {
            match dep {
                Dependency::Version(v) => {
                    table.insert(name, toml_edit::value(v));
                }
                Dependency::Path { path } => {
                    let mut inline = toml_edit::InlineTable::new();
                    inline.insert("path", path.into());
                    table.insert(name, toml_edit::Item::Value(toml_edit::Value::InlineTable(inline)));
                }

            }
        }
        
        fs::write(&toml_path, doc.to_string())?;
        Ok(())
    }

    /// Remove a dependency while strictly preserving user formatting and comments
    pub fn remove_dependency(dir: &Path, name: &str) -> Result<(), ManifestError> {
        let toml_path = dir.join("pace.toml");
        let content = fs::read_to_string(&toml_path)?;
        let mut doc = content.parse::<toml_edit::DocumentMut>().map_err(|e| ManifestError::EditError(e.to_string()))?;
        
        if let Some(deps) = doc.get_mut("dependencies") {
            if let Some(table) = deps.as_table_mut() {
                table.remove(name);
            }
        }
        
        fs::write(&toml_path, doc.to_string())?;
        Ok(())
    }

    /// Update a dependency version while strictly preserving user formatting and comments
    pub fn update_dependency_version(dir: &Path, name: &str, new_version: &str) -> Result<(), ManifestError> {
        let toml_path = dir.join("pace.toml");
        let content = fs::read_to_string(&toml_path)?;
        let mut doc = content.parse::<toml_edit::DocumentMut>().map_err(|e| ManifestError::EditError(e.to_string()))?;
        
        if let Some(deps) = doc.get_mut("dependencies") {
            if let Some(table) = deps.as_table_mut() {
                if table.contains_key(name) {
                    // Only update if it's a simple version string, don't overwrite { path = "..." }
                    if table[name].is_str() {
                        table[name] = toml_edit::value(new_version);
                    }
                }
            }
        }
        
        fs::write(&toml_path, doc.to_string())?;
        Ok(())
    }
}
