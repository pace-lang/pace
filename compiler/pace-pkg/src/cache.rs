use flate2::read::GzDecoder;
use reqwest::Client;
use std::fs;
use std::path::PathBuf;
use tar::Archive;

pub struct CacheManager {
    cache_dir: PathBuf,
}

impl CacheManager {
    pub fn new() -> Self {
        let cache_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".pace")
            .join("cache");
        fs::create_dir_all(&cache_dir).unwrap();
        Self { cache_dir }
    }

    pub fn get_package_path(&self, name: &str, version: &str) -> PathBuf {
        self.cache_dir.join(format!("{}-{}", name, version))
    }

    pub async fn download_and_extract(
        &self,
        name: &str,
        version: &str,
        _tarball_sha256: &str,
    ) -> Result<PathBuf, String> {
        let pkg_dir = self.get_package_path(name, version);
        if pkg_dir.exists() {
            return Ok(pkg_dir); // Already cached
        }

        // Use configurable registry or default
        let registry_url = std::env::var("PACE_REGISTRY_URL")
            .unwrap_or_else(|_| "http://localhost:3000/api/packages".to_string());
        
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

        let tar = GzDecoder::new(&bytes[..]);
        let mut archive = Archive::new(tar);

        archive
            .unpack(&pkg_dir)
            .map_err(|e| format!("Failed to extract {}: {}", name, e))?;

        Ok(pkg_dir)
    }
}
