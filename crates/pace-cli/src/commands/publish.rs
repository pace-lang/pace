use miette::Result;
use pace_pkg::manifest::PaceToml;
use std::io::Write;
use walkdir::WalkDir;
use std::fs::File;
use pace_driver::CompilerSession;
use crate::utils::resolve_file;

pub fn execute(session: &CompilerSession, dry_run: bool) -> Result<()> {
    let current_dir = std::env::current_dir().map_err(|e| miette::miette!("Failed to get current dir: {}", e))?;
    
    // Run compiler check before packaging
    let resolved_file = resolve_file(None)?;
    println!("🧪 Checking {} before publishing...", resolved_file);
    session.check_file(&resolved_file)?;
    println!("✅ Check passed for main entry point.");

    // Check tests/ and examples/
    for dir in &["tests", "examples"] {
        let dir_path = current_dir.join(dir);
        if dir_path.exists() && dir_path.is_dir() {
            for entry in WalkDir::new(&dir_path) {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("pace") {
                        let relative_str = path.strip_prefix(&current_dir).unwrap().to_string_lossy();
                        println!("🧪 Checking {} before publishing...", relative_str);
                        let path_str = path.to_string_lossy();
                        session.check_file(&path_str)?;
                    }
                }
            }
        }
    }
    println!("✅ All additional checks passed!");
    
    // Load manifest
    let manifest = PaceToml::load_from_dir(&current_dir)
        .map_err(|e| miette::miette!("Failed to load pace.toml. Are you in a pace project? {}", e))?;
        
    crate::utils::check_sdk_compatibility(&manifest)?;
    
    let pkg_name = &manifest.package.name;
    let pkg_version = &manifest.package.version;
    
    let pkg_desc = manifest.package.description.clone()
        .ok_or_else(|| miette::miette!("Missing 'description' in pace.toml"))?;
    if pkg_desc.is_empty() {
        return Err(miette::miette!("'description' in pace.toml cannot be empty"));
    }
    
    let _pkg_license = manifest.package.license.clone()
        .ok_or_else(|| miette::miette!("Missing 'license' in pace.toml"))?;
        
    let pkg_authors = manifest.package.authors.clone()
        .ok_or_else(|| miette::miette!("Missing 'authors' in pace.toml"))?;
    if pkg_authors.is_empty() {
        return Err(miette::miette!("'authors' in pace.toml cannot be empty"));
    }
        
    let _pkg_repo = manifest.package.repository.clone()
        .ok_or_else(|| miette::miette!("Missing 'repository' in pace.toml"))?;

    if dry_run {
        println!("🔍 Dry run: Packaging {} v{}...", pkg_name, pkg_version);
        println!("Files to be included:");
    } else {
        println!("📦 Packaging {} v{}...", pkg_name, pkg_version);
    }

    let mut files_to_include = Vec::new();
    let mut total_size = 0;

    for entry in WalkDir::new(&current_dir) {
        let entry = entry.map_err(|e| miette::miette!("Failed to traverse dir: {}", e))?;
        let path = entry.path();

        if path.is_file() {
            let relative = path.strip_prefix(&current_dir).unwrap();
            let relative_str = relative.to_string_lossy().to_string();

            // Filter logic
            let is_allowed = relative_str == "pace.toml"
                || relative_str == "README.md"
                || relative_str == "CHANGELOG.md"
                || relative_str == "LICENSE"
                || relative_str.starts_with("src/")
                || relative_str.starts_with("tests/");

            if is_allowed {
                if let Ok(metadata) = std::fs::metadata(path) {
                    files_to_include.push((relative.to_owned(), path.to_owned()));
                    total_size += metadata.len();
                    if dry_run {
                        println!("  - {} ({} bytes)", relative_str, metadata.len());
                    }
                }
            }
        }
    }

    if dry_run {
        println!("Total size: {} bytes", total_size);
        println!("✅ Dry run completed.");
        return Ok(());
    }

    // Create tarball in memory
    let mut tarball_data = Vec::new();
    {
        let enc = flate2::write::GzEncoder::new(&mut tarball_data, flate2::Compression::default());
        let mut builder = tar::Builder::new(enc);
        
        for (relative, absolute) in files_to_include {
            let mut f = File::open(&absolute).map_err(|e| miette::miette!("Failed to open file {:?}: {}", absolute, e))?;
            builder.append_file(relative, &mut f).map_err(|e| miette::miette!("Failed to append to tarball: {}", e))?;
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
    
    let registry_url = std::env::var("PACE_REGISTRY_URL").unwrap_or_else(|_| "https://registry.pace.dev".to_string());
    let url = format!("{}/api/packages/{}/publish", registry_url, pkg_name);
    
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
        
    if resp.status() != 201 && resp.status() != 200 {
        return Err(miette::miette!("Registry rejected publish: status {}", resp.status()));
    }
    
    println!("✅ Successfully published {} v{}!", pkg_name, pkg_version);
    Ok(())
}
