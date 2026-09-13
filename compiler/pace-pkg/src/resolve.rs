use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::PaceToml;

#[derive(Debug, Serialize, Deserialize)]
pub struct PaceLock {
    pub packages: HashMap<String, LockedPackage>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LockedPackage {
    pub version: String,
    pub source: Option<String>,
    pub checksum: Option<String>,
}

pub struct DependencyResolver {
    client: Client,
    registry_url: String,
    resolved: HashMap<String, LockedPackage>,
}

#[derive(Deserialize)]
struct RegistryResponse {
    latest_version: Option<String>,
    version_info: Vec<VersionInfo>,
}

#[derive(Deserialize)]
struct VersionInfo {
    version: String,
    dependencies: HashMap<String, String>,
    tarball_sha256: Option<String>,
}

impl DependencyResolver {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            registry_url: "http://localhost:3000/api/packages".to_string(),
            resolved: HashMap::new(),
        }
    }

    pub fn resolve(&mut self, toml: &PaceToml) -> Result<PaceLock, String> {
        let mut queue = Vec::new();
        for (name, dep) in &toml.dependencies {
            queue.push((
                name.clone(),
                dep.version().map(|s| s.to_string()),
                dep.path().map(|s| s.to_string()),
            ));
        }

        while let Some((name, version_req, path)) = queue.pop() {
            if self.resolved.contains_key(&name) {
                continue; // If already resolved, assume it's valid for now.
            }

            if let Some(p) = path {
                self.resolved.insert(
                    name,
                    LockedPackage {
                        version: version_req.unwrap_or_else(|| "0.1.0".to_string()),
                        source: Some(format!("local+{}", p)),
                        checksum: None,
                    },
                );
                continue;
            }

            let url = format!("{}/{}", self.registry_url, name);
            let resp = self.client.get(&url).send().map_err(|e| e.to_string())?;
            if !resp.status().is_success() {
                return Err(format!("Failed to find package {} in registry", name));
            }
            let info: RegistryResponse = resp.json().map_err(|e| e.to_string())?;

            // Simplified for first iteration: pick latest
            let latest = info
                .latest_version
                .ok_or_else(|| format!("No versions found for {}", name))?;
            let v_info = info
                .version_info
                .into_iter()
                .find(|v| v.version == latest)
                .unwrap();

            self.resolved.insert(
                name.clone(),
                LockedPackage {
                    version: latest,
                    source: Some("registry".to_string()),
                    checksum: v_info.tarball_sha256,
                },
            );

            for (dep_name, dep_ver) in v_info.dependencies {
                queue.push((dep_name, Some(dep_ver), None));
            }
        }

        Ok(PaceLock {
            packages: self.resolved.clone(),
        })
    }

    pub fn write_lockfile(&self, path: &Path, lock: &PaceLock) -> Result<(), String> {
        let toml_str = toml::to_string(lock).map_err(|e| e.to_string())?;
        fs::write(path, toml_str).map_err(|e| e.to_string())
    }

    pub fn read_lockfile(path: &Path) -> Result<PaceLock, String> {
        let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
        toml::from_str(&content).map_err(|e| e.to_string())
    }
}
