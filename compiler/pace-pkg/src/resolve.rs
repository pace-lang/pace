use reqwest::Client;
use semver::{Version, VersionReq};
use std::env;
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

#[allow(dead_code)]
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
            registry_url: env::var("PACE_REGISTRY_URL").unwrap_or_else(|_| "https://registry.pace-lang.org/api/packages".to_string()),
            resolved: HashMap::new(),
        }
    }

    pub async fn resolve(&mut self, toml: &PaceToml) -> Result<PaceLock, String> {
        let mut queue = Vec::new();
        for (name, dep) in &toml.dependencies {
            queue.push((
                name.clone(),
                dep.version().map(|s| s.to_string()),
                dep.path().map(|s| s.to_string()),
            ));
        }

        while let Some((name, version_req_str, path)) = queue.pop() {
            let req = version_req_str
                .as_deref()
                .map(|s| VersionReq::parse(s).map_err(|e| format!("Invalid version requirement '{}' for {}: {}", s, name, e)))
                .transpose()?;

            if let Some(existing) = self.resolved.get(&name) {
                if let Some(req) = &req {
                    let existing_ver = existing.version.as_str();
                    if let Ok(ver) = Version::parse(existing_ver) {
                        if !req.matches(&ver) {
                            return Err(format!("Conflict detected for package {}: already resolved to {} which does not satisfy {}", name, existing_ver, req));
                        }
                    }
                }
                continue;
            }

            if let Some(p) = path {
                self.resolved.insert(
                    name,
                    LockedPackage {
                        version: version_req_str.unwrap_or_else(|| "0.1.0".to_string()),
                        source: Some(format!("local+{}", p)),
                        checksum: None,
                    },
                );
                continue;
            }

            let url = format!("{}/{}", self.registry_url, name);
            let resp = self.client.get(&url).send().await.map_err(|e| e.to_string())?;
            if !resp.status().is_success() {
                return Err(format!("Failed to find package {} in registry", name));
            }
            let info: RegistryResponse = resp.json().await.map_err(|e| e.to_string())?;

            let mut best_match: Option<VersionInfo> = None;
            let mut highest_ver: Option<Version> = None;

            for v_info in info.version_info {
                let ver = match Version::parse(&v_info.version) {
                    Ok(v) => v,
                    Err(_) => continue, // skip invalid versions in registry
                };
                
                if let Some(r) = &req {
                    if !r.matches(&ver) {
                        continue;
                    }
                }
                
                if highest_ver.is_none() || ver > *highest_ver.as_ref().unwrap() {
                    highest_ver = Some(ver);
                    best_match = Some(v_info);
                }
            }

            let best_match = best_match.ok_or_else(|| format!("No compatible versions found for {} satisfying {:?}", name, version_req_str))?;

            self.resolved.insert(
                name.clone(),
                LockedPackage {
                    version: best_match.version.clone(),
                    source: Some("registry".to_string()),
                    checksum: best_match.tarball_sha256.clone(),
                },
            );

            for (dep_name, dep_ver) in best_match.dependencies {
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
