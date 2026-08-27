
use std::path::{Path, PathBuf};
use std::process::Command;
use miette::{miette, Result};
use crate::manifest::{Dependency, PaceToml};
use crate::lockfile::{LockedPackage, PaceLock};

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
        std::fs::create_dir_all(&registry_cache_dir).map_err(|e| miette!("Failed to create registry cache dir: {}", e))?;
        Ok(Self { registry_cache_dir })
    }

    /// Fetches dependencies for a given project directory, updating the lockfile.
    pub fn fetch(&self, project_dir: &Path) -> Result<()> {
        let manifest = PaceToml::load_from_dir(project_dir)
            .map_err(|e| miette!("Failed to load pace.toml: {}", e))?;
        
        let mut lock = PaceLock::load_from_dir(project_dir)
            .unwrap_or_else(|_| None)
            .unwrap_or_default();

        for (pkg_name, dep) in &manifest.dependencies {
            match dep {

                Dependency::Path { path } => {
                    let full_path = project_dir.join(path).canonicalize()
                        .map_err(|e| miette!("Failed to resolve local path {}: {}", path, e))?;
                    lock.packages.insert(pkg_name.clone(), LockedPackage {
                        version: None,
                        path: Some(full_path.to_string_lossy().to_string()),
                    });
                }
                Dependency::Version(v) => {
                    let locked = self.fetch_registry(pkg_name, v)?;
                    lock.packages.insert(pkg_name.clone(), locked);
                }
            }
        }

        lock.save_to_dir(project_dir).map_err(|e| miette!("Failed to save lockfile: {}", e))?;
        Ok(())
    }



    fn fetch_registry(&self, pkg_name: &str, version: &str) -> Result<LockedPackage> {
        let cache_name = format!("{}-{}", pkg_name, version);
        let final_path = self.registry_cache_dir.join(&cache_name);

        if !final_path.exists() {
            println!("⬇️  Downloading {} v{} from registry...", pkg_name, version);
            let registry_url = std::env::var("PACE_REGISTRY_URL").unwrap_or_else(|_| "https://registry.pace.dev".to_string());
            let url = format!("{}/api/packages/{}/download/{}", registry_url, pkg_name, version);
            let resp = ureq::get(&url).call().map_err(|e| miette!("Failed to download package: {}", e))?;
            
            if resp.status() != 200 {
                return Err(miette!("Failed to download package, status: {}", resp.status()));
            }

            let temp_dir = tempfile::tempdir().map_err(|e| miette!("Failed to create temp dir: {}", e))?;
            let temp_path = temp_dir.path();

            let reader = resp.into_body().into_reader();
            let tar = flate2::read::GzDecoder::new(reader);
            let mut archive = tar::Archive::new(tar);
            
            archive.unpack(temp_path).map_err(|e| miette!("Failed to unpack tarball: {}", e))?;

            std::fs::rename(temp_path, &final_path).or_else(|_| {
                Command::new("cp")
                    .arg("-r")
                    .arg(temp_path)
                    .arg(&final_path)
                    .status()
                    .map(|_| ())
            }).map_err(|e| miette!("Failed to move unpacked package to cache: {:?}", e))?;
        }

        Ok(LockedPackage {
            version: Some(version.to_string()),
            path: Some(final_path.to_string_lossy().to_string()),
        })
    }
}
