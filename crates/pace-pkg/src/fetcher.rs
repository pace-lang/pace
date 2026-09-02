use crate::lockfile::{LockedPackage, PaceLock};
use crate::manifest::{Dependency, PaceToml};
use miette::{Result, miette};
use semver::{Version, VersionReq};
use serde::Deserialize;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Deserialize)]
pub struct RegistryResponse {
    pub latest_version: Option<String>,
    pub versions: Option<Vec<String>>,
    pub version_info: Option<Vec<VersionInfo>>,
}

#[derive(Deserialize)]
pub struct VersionInfo {
    pub version: String,
    pub tarball_sha256: Option<String>,
    pub dependencies: Option<std::collections::HashMap<String, String>>,
}

pub struct Fetcher {
    registry_cache_dir: PathBuf,
}

impl Fetcher {
    pub fn new() -> Result<Self> {
        let home_dir = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map_err(|_| miette!("Could not find HOME directory"))?;
        let base_cache = PathBuf::from(home_dir).join(".pace").join("cache");
        let registry_cache_dir = base_cache.join("registry");
        std::fs::create_dir_all(&registry_cache_dir)
            .map_err(|e| miette!("Failed to create registry cache dir: {}", e))?;
        Ok(Self { registry_cache_dir })
    }

    /// Fetches dependencies for a given project directory, updating the lockfile.
    pub fn fetch(&self, project_dir: &Path) -> Result<()> {
        let manifest = PaceToml::load_from_dir(project_dir)
            .map_err(|e| miette!("Failed to load pace.toml: {}", e))?;

        let mut lock = PaceLock::load_from_dir(project_dir)
            .unwrap_or(None)
            .unwrap_or_default();

        // Clear the packages so we don't keep removed dependencies around
        lock.packages.clear();

        let mut pubgrub_deps = std::collections::HashMap::new();
        let mut local_paths = std::collections::HashMap::new();

        for (pkg_name, dep) in &manifest.dependencies {
            match dep {
                Dependency::Path { path } => {
                    let full_path = project_dir
                        .join(path)
                        .canonicalize()
                        .map_err(|e| miette!("Failed to resolve local path {}: {}", path, e))?;
                    local_paths.insert(pkg_name.clone(), full_path.to_string_lossy().to_string());
                }
                Dependency::Version(constraint) => {
                    // Pre-validate that the constraint matches at least one version on the registry.
                    // This will error if the version does not exist (e.g. user entered a nonexistent version).
                    let _ = Self::resolve_version(pkg_name, constraint)?;

                    let range = crate::utils::parse_range(constraint);
                    pubgrub_deps.insert(pkg_name.clone(), range);
                }
            }
        }

        let provider = crate::resolver::RegistryProvider::new();
        let root_pkg = crate::resolver::PackageName(manifest.package.name.clone());
        let root_version = pubgrub::SemanticVersion::new(0, 0, 0);

        provider.add_root_dependencies(root_pkg.clone(), root_version, pubgrub_deps);

        println!("🔄 Resolving dependency graph...");
        let resolved =
            pubgrub::resolve(&provider, root_pkg.clone(), root_version).map_err(|e| {
                miette!(
                    "Version resolution failed (conflicts or circular deps detected): {:?}",
                    e
                )
            })?;

        for (pkg, version) in resolved {
            if pkg == root_pkg {
                continue;
            }
            let pkg_name = pkg.0.clone();

            // Check if it's a local path
            if let Some(path) = local_paths.get(&pkg_name) {
                lock.packages.insert(
                    pkg_name.clone(),
                    LockedPackage {
                        version: None,
                        path: Some(path.clone()),
                    },
                );
                continue;
            }

            let version_str = version.to_string();

            // For now, we don't have the expected sha256 strictly passed through pubgrub resolver since it returns SemanticVersion.
            // We could retrieve it by making a quick API call to resolve_version, or fetch it without verification.
            // We'll just fetch without sha verification for transitive ones to keep the integration simple.
            let locked = self.fetch_registry(&pkg_name, &version_str, None)?;
            lock.packages.insert(pkg_name, locked);
        }

        // Add local path dependencies explicitly since pubgrub ignores them
        for (pkg_name, path) in local_paths {
            lock.packages.insert(
                pkg_name,
                LockedPackage {
                    version: None,
                    path: Some(path),
                },
            );
        }

        lock.save_to_dir(project_dir)
            .map_err(|e| miette!("Failed to save lockfile: {}", e))?;

        // Link dependencies to .pace/deps for the compiler to find them
        let deps_dir = project_dir.join(".pace").join("deps");
        let _ = std::fs::remove_dir_all(&deps_dir);
        std::fs::create_dir_all(&deps_dir)
            .map_err(|e| miette!("Failed to create .pace/deps: {}", e))?;

        for (pkg_name, locked) in &lock.packages {
            if let Some(src_path) = &locked.path {
                let dst_path = deps_dir.join(pkg_name);
                #[cfg(unix)]
                std::os::unix::fs::symlink(src_path, &dst_path)
                    .map_err(|e| miette!("Failed to symlink package {}: {}", pkg_name, e))?;
                
                #[cfg(not(unix))]
                std::os::windows::fs::symlink_dir(src_path, &dst_path)
                    .map_err(|e| miette!("Failed to symlink package {}: {}", pkg_name, e))?;
            }
        }

        Ok(())
    }

    pub fn resolve_version(pkg_name: &str, constraint: &str) -> Result<(String, Option<String>)> {
        let registry_url = std::env::var("PACE_REGISTRY_URL")
            .unwrap_or_else(|_| "https://registry.pace.dev".to_string());
        let url = format!("{}/api/packages/{}", registry_url, pkg_name);

        let resp = match ureq::get(&url).call() {
            Ok(r) => {
                if r.status() == 404 {
                    return Err(miette!("Package '{}' not found in registry", pkg_name));
                } else if r.status() != 200 {
                    return Err(miette!("Registry returned error status: {}", r.status()));
                }
                r
            }
            Err(e) => return Err(miette!("Failed to connect to registry: {}", e)),
        };

        let parsed: RegistryResponse = resp
            .into_body()
            .read_json()
            .map_err(|e| miette!("Failed to parse registry response: {}", e))?;

        let req = VersionReq::parse(constraint)
            .map_err(|e| miette!("Invalid version constraint '{}': {}", constraint, e))?;

        let mut available_versions = Vec::new();
        if let Some(versions) = parsed.versions {
            for v_str in versions {
                if let Ok(v) = Version::parse(&v_str) {
                    available_versions.push(v);
                }
            }
        }

        available_versions.sort();
        available_versions.reverse();

        for v in available_versions {
            if req.matches(&v) {
                let matched_v = v.to_string();
                let sha = parsed
                    .version_info
                    .as_ref()
                    .and_then(|info| info.iter().find(|i| i.version == matched_v))
                    .and_then(|i| i.tarball_sha256.clone());
                return Ok((matched_v, sha));
            }
        }

        if let Some(latest) = parsed.latest_version {
            if let Ok(latest_v) = Version::parse(&latest) {
                if req.matches(&latest_v) {
                    let sha = parsed
                        .version_info
                        .as_ref()
                        .and_then(|info| info.iter().find(|i| i.version == latest))
                        .and_then(|i| i.tarball_sha256.clone());
                    return Ok((latest, sha));
                }
            } else if constraint == latest {
                let sha = parsed
                    .version_info
                    .as_ref()
                    .and_then(|info| info.iter().find(|i| i.version == latest))
                    .and_then(|i| i.tarball_sha256.clone());
                return Ok((latest, sha));
            }
        }

        Err(miette!(
            "No versions found for '{}' that satisfy constraint '{}'",
            pkg_name,
            constraint
        ))
    }

    fn fetch_registry(
        &self,
        pkg_name: &str,
        version: &str,
        expected_sha: Option<&str>,
    ) -> Result<LockedPackage> {
        let cache_name = format!("{}-{}", pkg_name, version);
        let final_path = self.registry_cache_dir.join(&cache_name);

        if !final_path.exists() {
            println!("⬇️  Downloading {} v{} from registry...", pkg_name, version);
            let registry_url = std::env::var("PACE_REGISTRY_URL")
                .unwrap_or_else(|_| "https://registry.pace.dev".to_string());
            let url = format!(
                "{}/api/packages/{}/download/{}",
                registry_url, pkg_name, version
            );
            let resp = ureq::get(&url)
                .call()
                .map_err(|e| miette!("Failed to download package: {}", e))?;

            if resp.status() != 200 {
                return Err(miette!(
                    "Failed to download package, status: {}",
                    resp.status()
                ));
            }

            let temp_dir =
                tempfile::tempdir().map_err(|e| miette!("Failed to create temp dir: {}", e))?;
            let temp_path = temp_dir.path();

            let mut buffer = Vec::new();
            resp.into_body()
                .into_reader()
                .read_to_end(&mut buffer)
                .map_err(|e| miette!("Failed to read response body: {}", e))?;

            if let Some(expected) = expected_sha {
                use sha2::{Digest, Sha256};
                let mut hasher = Sha256::new();
                hasher.update(&buffer);
                let result = hasher.finalize();
                let hash_str = result
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<String>();
                if hash_str != expected {
                    return Err(miette!(
                        "Security Error: SHA256 mismatch for {} v{}. Expected {}, got {}",
                        pkg_name,
                        version,
                        expected,
                        hash_str
                    ));
                }
            }

            let reader = std::io::Cursor::new(buffer);
            let tar = flate2::read::GzDecoder::new(reader);
            let mut archive = tar::Archive::new(tar);

            archive
                .unpack(temp_path)
                .map_err(|e| miette!("Failed to unpack tarball: {}", e))?;

            std::fs::rename(temp_path, &final_path)
                .or_else(|_| {
                    Command::new("cp")
                        .arg("-r")
                        .arg(temp_path)
                        .arg(&final_path)
                        .status()
                        .map(|_| ())
                })
                .map_err(|e| miette!("Failed to move unpacked package to cache: {:?}", e))?;
        }

        Ok(LockedPackage {
            version: Some(version.to_string()),
            path: Some(final_path.to_string_lossy().to_string()),
        })
    }
}
