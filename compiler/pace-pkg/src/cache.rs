use flate2::read::GzDecoder;
use reqwest::Client;
use std::fs;
use std::path::PathBuf;
use tar::Archive;

pub struct CacheManager {
    cache_dir: PathBuf,
}

impl Default for CacheManager {
    fn default() -> Self {
        Self::new()
    }
}

impl CacheManager {
    pub fn new() -> Self {
        let cache_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".pace")
            .join("cache");
        if let Err(e) = fs::create_dir_all(&cache_dir) {
            eprintln!("Warning: Failed to create cache directory: {}", e);
        }
        Self { cache_dir }
    }

    pub fn get_package_path(&self, name: &str, version: &str) -> PathBuf {
        self.cache_dir.join(format!("{}-{}", name, version))
    }

    pub async fn download_and_extract(
        &self,
        name: &str,
        version: &str,
        tarball_sha256: &str,
    ) -> Result<PathBuf, String> {
        let pkg_dir = self.get_package_path(name, version);
        if pkg_dir.exists() {
            return Ok(pkg_dir); // Already cached
        }

        // Use configurable registry or default
        let registry_url = std::env::var("PACE_REGISTRY_URL")
            .unwrap_or_else(|_| "https://pace-registry.pace-lang.workers.dev/api/packages".to_string());

        let url = format!("{}/{}/download/{}", registry_url, name, version);
        let client = Client::new();
        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to download {}: {}", name, e))?;

        if !resp.status().is_success() {
            return Err(format!(
                "Failed to download {}: HTTP {}",
                name,
                resp.status()
            ));
        }

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| format!("Failed to read body: {}", e))?;

        if !tarball_sha256.is_empty() {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(&bytes);
            let calculated = format!("{:x}", hasher.finalize());
            if !calculated.eq_ignore_ascii_case(tarball_sha256) {
                return Err(format!(
                    "Checksum mismatch for package {}@{}: expected {}, got {}",
                    name, version, tarball_sha256, calculated
                ));
            }
        }

        let tar = GzDecoder::new(&bytes[..]);
        let mut archive = Archive::new(tar);

        let tmp_dir = self.cache_dir.join(format!("{}-{}.tmp", name, version));
        if tmp_dir.exists() {
            fs::remove_dir_all(&tmp_dir)
                .map_err(|e| format!("Failed to clear temp directory: {}", e))?;
        }

        archive
            .unpack(&tmp_dir)
            .map_err(|e| format!("Failed to extract {}: {}", name, e))?;

        fs::rename(&tmp_dir, &pkg_dir)
            .map_err(|e| format!("Failed to rename extracted directory: {}", e))?;

        Ok(pkg_dir)
    }
}
