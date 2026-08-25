use miette::Result;
use pace_pkg::manifest::PaceToml;
use std::io::Write;
use walkdir::WalkDir;
use std::fs::File;

pub fn execute() -> Result<()> {
    let current_dir = std::env::current_dir().map_err(|e| miette::miette!("Failed to get current dir: {}", e))?;
    
    // Load manifest
    let manifest = PaceToml::load_from_dir(&current_dir)
        .map_err(|e| miette::miette!("Failed to load pace.toml. Are you in a pace project? {}", e))?;
    
    let pkg_name = &manifest.package.name;
    let pkg_version = &manifest.package.version;
    let pkg_desc = manifest.package.description.clone().unwrap_or_else(|| String::new());

    println!("📦 Packaging {} v{}...", pkg_name, pkg_version);

    // Create tarball in memory
    let mut tarball_data = Vec::new();
    {
        let enc = flate2::write::GzEncoder::new(&mut tarball_data, flate2::Compression::default());
        let mut builder = tar::Builder::new(enc);
        
        for entry in WalkDir::new(&current_dir) {
            let entry = entry.map_err(|e| miette::miette!("Failed to traverse dir: {}", e))?;
            let path = entry.path();
            
            // Skip .git, target, .pace
            if path.components().any(|c| {
                let s = c.as_os_str().to_string_lossy();
                s == ".git" || s == "target" || s == ".pace"
            }) {
                continue;
            }

            if path.is_file() {
                let relative = path.strip_prefix(&current_dir).unwrap();
                let mut f = File::open(path).map_err(|e| miette::miette!("Failed to open file {:?}: {}", path, e))?;
                builder.append_file(relative, &mut f).map_err(|e| miette::miette!("Failed to append to tarball: {}", e))?;
            }
        }
        
        builder.into_inner().map_err(|e| miette::miette!("Failed to finish tar: {}", e))?
            .finish().map_err(|e| miette::miette!("Failed to finish gzip: {}", e))?;
    }
    
    println!("🚀 Uploading {} bytes to registry...", tarball_data.len());
    
    // Build multipart
    let boundary = "--------PaceRegistryBoundary";
    let mut body = Vec::new();
    
    // Version part
    write!(body, "--{}\r\nContent-Disposition: form-data; name=\"version\"\r\n\r\n{}\r\n", boundary, pkg_version).unwrap();
    // Description part
    write!(body, "--{}\r\nContent-Disposition: form-data; name=\"description\"\r\n\r\n{}\r\n", boundary, pkg_desc).unwrap();
    
    // Tarball part
    write!(body, "--{}\r\nContent-Disposition: form-data; name=\"tarball\"; filename=\"{}-{}.tar.gz\"\r\nContent-Type: application/gzip\r\n\r\n", boundary, pkg_name, pkg_version).unwrap();
    body.extend_from_slice(&tarball_data);
    write!(body, "\r\n--{}--\r\n", boundary).unwrap();
    
    let url = format!("http://localhost:3000/api/packages/{}/publish", pkg_name);
    
    // Read credentials token
    let home_dir = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let credentials_path = std::path::Path::new(&home_dir).join(".pace").join("credentials.toml");
    
    let token = if credentials_path.exists() {
        let content = std::fs::read_to_string(&credentials_path).unwrap_or_default();
        content.lines()
            .find(|l| l.starts_with("token = "))
            .and_then(|l| l.split('"').nth(1))
            .map(|s| s.to_string())
            .unwrap_or_else(|| "dev_token_123".to_string())
    } else {
        "dev_token_123".to_string()
    };

    let resp = match ureq::post(&url)
        .header("Authorization", &format!("Bearer {}", token))
        .header("Content-Type", &format!("multipart/form-data; boundary={}", boundary))
        .send(&body)
    {
        Ok(r) => {
            if r.status() != 200 {
                let text = r.into_body().read_to_string().unwrap_or_default();
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(err_msg) = json.get("error").and_then(|v| v.as_str()) {
                        return Err(miette::miette!("Registry rejected publish: {}", err_msg));
                    }
                }
                return Err(miette::miette!("Registry rejected publish: {}", text));
            }
            r
        }
        Err(e) => return Err(miette::miette!("Failed to publish to registry: {}", e)),
    };
        
    if resp.status() != 201 {
        return Err(miette::miette!("Registry rejected publish: status {}", resp.status()));
    }
    
    println!("✅ Successfully published {} v{}!", pkg_name, pkg_version);
    Ok(())
}
