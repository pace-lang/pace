
use std::path::{Path, PathBuf};
use std::process::Command;
use miette::{miette, Result};
use crate::manifest::{Dependency, PaceToml};
use crate::lockfile::{LockedPackage, PaceLock};

pub struct Fetcher {
    git_cache_dir: PathBuf,
    registry_cache_dir: PathBuf,
}

impl Fetcher {
    pub fn new() -> Result<Self> {
        let home_dir = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map_err(|_| miette!("Could not find HOME directory"))?;
        let base_cache = PathBuf::from(home_dir).join(".pace").join("cache");
        let git_cache_dir = base_cache.join("git");
        let registry_cache_dir = base_cache.join("registry");
        std::fs::create_dir_all(&git_cache_dir).map_err(|e| miette!("Failed to create git cache dir: {}", e))?;
        std::fs::create_dir_all(&registry_cache_dir).map_err(|e| miette!("Failed to create registry cache dir: {}", e))?;
        Ok(Self { git_cache_dir, registry_cache_dir })
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
                Dependency::Git { git, branch, rev } => {
                    let locked = self.fetch_git(pkg_name, git, branch.as_deref(), rev.as_deref())?;
                    lock.packages.insert(pkg_name.clone(), locked);
                }
                Dependency::Path { path } => {
                    let full_path = project_dir.join(path).canonicalize()
                        .map_err(|e| miette!("Failed to resolve local path {}: {}", path, e))?;
                    lock.packages.insert(pkg_name.clone(), LockedPackage {
                        version: None,
                        git: None,
                        rev: None,
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

    fn fetch_git(&self, _pkg_name: &str, url: &str, branch: Option<&str>, rev: Option<&str>) -> Result<LockedPackage> {
        // Create a temporary directory for cloning
        let temp_dir = tempfile::tempdir().map_err(|e| miette!("Failed to create temp dir: {}", e))?;
        let temp_path = temp_dir.path();

        // Git clone
        let mut cmd = Command::new("git");
        cmd.arg("clone").arg(url).arg(temp_path);
        
        if let Some(b) = branch {
            cmd.arg("--branch").arg(b);
        }
        
        let status = cmd.status().map_err(|e| miette!("Failed to execute git clone: {}", e))?;
        if !status.success() {
            return Err(miette!("git clone failed for URL: {}", url));
        }

        // Checkout specific revision if provided
        if let Some(r) = rev {
            let status = Command::new("git")
                .current_dir(temp_path)
                .arg("checkout")
                .arg(r)
                .status()
                .map_err(|e| miette!("Failed to execute git checkout: {}", e))?;
            if !status.success() {
                return Err(miette!("git checkout failed for rev: {}", r));
            }
        }

        // Get the actual commit hash
        let output = Command::new("git")
            .current_dir(temp_path)
            .arg("rev-parse")
            .arg("HEAD")
            .output()
            .map_err(|e| miette!("Failed to execute git rev-parse: {}", e))?;
        
        if !output.status.success() {
            return Err(miette!("git rev-parse failed"));
        }

        let commit_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Extract a safe repo name from the URL
        let repo_name = url.split('/').last().unwrap_or("unknown").trim_end_matches(".git");
        let cache_name = format!("{}-{}", repo_name, commit_hash);
        let final_path = self.git_cache_dir.join(&cache_name);

        // Move to final path if it doesn't exist
        if !final_path.exists() {
            // Because tempdir might be on a different mount point, we use a recursive copy or fs::rename
            // In many OSes fs::rename works across the same filesystem, but to be safe:
            std::fs::rename(temp_path, &final_path).or_else(|_| {
                // fallback to a simple cp -r if rename fails (cross-device link)
                Command::new("cp")
                    .arg("-r")
                    .arg(temp_path)
                    .arg(&final_path)
                    .status()
                    .map(|_| ())
            }).map_err(|e| miette!("Failed to move repository to cache: {:?}", e))?;
        }

        Ok(LockedPackage {
            version: None,
            git: Some(url.to_string()),
            rev: Some(commit_hash),
            path: Some(final_path.to_string_lossy().to_string()),
        })
    }

    fn fetch_registry(&self, pkg_name: &str, version: &str) -> Result<LockedPackage> {
        let cache_name = format!("{}-{}", pkg_name, version);
        let final_path = self.registry_cache_dir.join(&cache_name);

        if !final_path.exists() {
            println!("⬇️  Downloading {} v{} from registry...", pkg_name, version);
            let url = format!("http://localhost:3000/api/packages/{}/download/{}", pkg_name, version);
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
            git: None,
            rev: None,
            path: Some(final_path.to_string_lossy().to_string()),
        })
    }
}
