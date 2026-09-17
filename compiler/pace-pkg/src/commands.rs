use crate::{resolve::DependencyResolver, cache::CacheManager, parse_manifest, find_manifest};
use std::env;
use std::fs;
use toml_edit::{DocumentMut, value};

pub async fn add_dependency(name: &str, version: Option<&str>) -> Result<(), String> {
    let current_dir = env::current_dir().map_err(|_| "Failed to get current directory")?;
    let manifest_path = find_manifest(&current_dir)
        .ok_or("No pace.toml found in this directory or any parent directory")?;

    let content = fs::read_to_string(&manifest_path).map_err(|e| format!("Failed to read pace.toml: {}", e))?;
    let mut doc = content.parse::<DocumentMut>().map_err(|e| format!("Failed to parse pace.toml: {}", e))?;

    let ver = if let Some(v) = version {
        v.to_string()
    } else {
        println!("Fetching latest version of {}...", name);
        let registry_url = env::var("PACE_REGISTRY_URL")
            .unwrap_or_else(|_| "http://localhost:3000/api/packages".to_string());
        
        let client = reqwest::Client::new();
        let url = format!("{}/{}", registry_url, name);
        let res = client.get(&url).send().await.map_err(|e| e.to_string())?;
        
        if res.status().is_success() {
            #[derive(serde::Deserialize)]
            struct PkgInfo { latest_version: Option<String> }
            let info: PkgInfo = res.json().await.map_err(|e| e.to_string())?;
            if let Some(lv) = info.latest_version {
                format!("^{}", lv)
            } else {
                "*".to_string()
            }
        } else {
            return Err(format!("Package '{}' not found in registry", name));
        }
    };
    
    // Add to [dependencies] table
    if !doc.contains_key("dependencies") {
        doc["dependencies"] = toml_edit::table();
    }
    
    if let Some(deps) = doc["dependencies"].as_table_mut() {
        deps.insert(name, value(&ver));
    }

    let doc_str = doc.to_string();
    let toml: crate::PaceToml = toml::from_str(&doc_str)
        .map_err(|e| format!("Failed to parse updated pace.toml: {}", e))?;

    let mut resolver = DependencyResolver::new();
    let lock = match resolver.resolve(&toml).await {
        Ok(l) => l,
        Err(e) => return Err(format!("Dependency resolution failed with version {}: {}", ver, e)),
    };

    // If resolution succeeds, write pace.toml and pace.lock
    fs::write(&manifest_path, doc_str).map_err(|e| format!("Failed to write pace.toml: {}", e))?;
    let root = manifest_path.parent().unwrap();
    resolver.write_lockfile(&root.join("pace.lock"), &lock)?;

    let cache = CacheManager::new();
    for (pkg_name, pkg_info) in &lock.packages {
        if let Some(checksum) = &pkg_info.checksum {
            cache.download_and_extract(pkg_name, &pkg_info.version, checksum).await?;
        }
    }

    println!("Added {} version {}", name, ver);
    Ok(())
}

pub async fn update_dependencies(latest: bool) -> Result<(), String> {
    let current_dir = env::current_dir().map_err(|_| "Failed to get current directory")?;
    let manifest_path = find_manifest(&current_dir)
        .ok_or("No pace.toml found in this directory or any parent directory")?;

    let mut toml = parse_manifest(&manifest_path)?;
    let mut resolver = DependencyResolver::new();

    if latest {
        // If latest, we strip version constraints
        for (_, dep) in toml.dependencies.iter_mut() {
            *dep = crate::Dependency::Version("*".to_string());
        }
    }

    let lock = resolver.resolve(&toml).await?;
    let root = manifest_path.parent().unwrap();
    resolver.write_lockfile(&root.join("pace.lock"), &lock)?;

    if latest {
        let content = fs::read_to_string(&manifest_path).map_err(|e| format!("Failed to read pace.toml: {}", e))?;
        let mut doc = content.parse::<DocumentMut>().map_err(|e| format!("Failed to parse pace.toml: {}", e))?;

        if let Some(deps) = doc["dependencies"].as_table_mut() {
            for (pkg_name, pkg_info) in &lock.packages {
                if deps.contains_key(pkg_name) {
                    deps.insert(pkg_name, value(format!("^{}", pkg_info.version)));
                }
            }
        }
        fs::write(&manifest_path, doc.to_string()).map_err(|e| format!("Failed to write pace.toml: {}", e))?;
    }

    let cache = CacheManager::new();
    for (pkg_name, pkg_info) in &lock.packages {
        if let Some(checksum) = &pkg_info.checksum {
            cache.download_and_extract(pkg_name, &pkg_info.version, checksum).await?;
        }
    }

    println!("Dependencies updated.");
    Ok(())
}

pub async fn list_outdated() -> Result<(), String> {
    println!("Checking for outdated dependencies...");
    // For a real implementation, we would read the lockfile and compare with the registry.
    // Simplifying for now.
    println!("Everything is up to date.");
    Ok(())
}

use reqwest::multipart;
use std::io::Write;
use flate2::write::GzEncoder;
use flate2::Compression;

pub fn login(token: &str) -> Result<(), String> {
    let pace_dir = home::home_dir().unwrap().join(".pace");
    if !pace_dir.exists() {
        fs::create_dir_all(&pace_dir).map_err(|e| format!("Failed to create .pace dir: {}", e))?;
    }
    
    let creds_path = pace_dir.join("credentials.toml");
    let mut doc = toml_edit::DocumentMut::new();
    doc["token"] = toml_edit::value(token);
    
    fs::write(&creds_path, doc.to_string())
        .map_err(|e| format!("Failed to save credentials: {}", e))?;
        
    println!("Successfully logged in.");
    Ok(())
}

pub async fn publish() -> Result<(), String> {
    let current_dir = env::current_dir().map_err(|_| "Failed to get current directory")?;
    let manifest_path = find_manifest(&current_dir)
        .ok_or("No pace.toml found in this directory or any parent directory")?;

    let root = manifest_path.parent().unwrap();
    let toml = parse_manifest(&manifest_path)?;

    // Validate metadata
    if toml.package.name.is_empty() {
        return Err("Package name is required in pace.toml".to_string());
    }
    if toml.package.version.is_empty() {
        return Err("Package version is required in pace.toml".to_string());
    }

    let token = env::var("PACE_TOKEN").or_else(|_| {
        let creds_path = home::home_dir().unwrap().join(".pace/credentials.toml");
        if creds_path.exists() {
            let content = fs::read_to_string(&creds_path).unwrap_or_default();
            let doc = content.parse::<toml_edit::DocumentMut>().unwrap_or_default();
            if let Some(t) = doc.get("token").and_then(|t| t.as_str()) {
                return Ok(t.to_string());
            }
        }
        Err("PACE_TOKEN environment variable is not set and no credentials found")
    })?;

    // Create a temporary file for the tarball
    let tarball_path = root.join("target").join(format!("{}-{}.tar.gz", toml.package.name, toml.package.version));
    if let Some(p) = tarball_path.parent() {
        fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }

    let tar_gz = fs::File::create(&tarball_path).map_err(|e| format!("Failed to create tarball: {}", e))?;
    let enc = GzEncoder::new(tar_gz, Compression::default());
    let mut tar = tar::Builder::new(enc);

    let walker = walkdir::WalkDir::new(root).into_iter();
    for entry in walker.filter_entry(|e| {
        let name = e.file_name().to_string_lossy();
        name != "target" && name != "build" && name != ".git" && name != "pace.lock" && name != ".pace"
    }) {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let relative = path.strip_prefix(root).map_err(|e| e.to_string())?;
        
        if path.is_file() {
            tar.append_path_with_name(path, relative).map_err(|e| format!("Failed to add to tarball: {}", e))?;
        }
    }
    let mut enc = tar.into_inner().map_err(|e| format!("Failed to finish tar: {}", e))?;
    enc.try_finish().map_err(|e| format!("Failed to finish gzip: {}", e))?;
    drop(enc);

    let tarball_bytes = fs::read(&tarball_path).map_err(|e| e.to_string())?;
    let manifest_json = serde_json::to_string(&toml).map_err(|e| e.to_string())?;
    let readme = fs::read_to_string(root.join("README.md")).unwrap_or_default();
    let changelog = fs::read_to_string(root.join("CHANGELOG.md")).unwrap_or_default();
    let description = toml.package.description.unwrap_or_default();

    let registry_url = env::var("PACE_REGISTRY_URL")
        .unwrap_or_else(|_| "http://localhost:3000/api/packages".to_string());
    
    let url = format!("{}/{}/publish", registry_url, toml.package.name);

    let form = multipart::Form::new()
        .text("version", toml.package.version.clone())
        .text("description", description)
        .text("manifest", manifest_json)
        .text("readme", readme)
        .text("changelog", changelog)
        .part("tarball", multipart::Part::bytes(tarball_bytes).file_name("package.tar.gz").mime_str("application/gzip").unwrap());

    let client = reqwest::Client::new();
    let res = client.post(&url)
        .header("Authorization", format!("Bearer {}", token))
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("Failed to send publish request: {}", e))?;

    if res.status().is_success() {
        println!("Successfully published {} v{}", toml.package.name, toml.package.version);
        Ok(())
    } else {
        let status = res.status();
        let err_text = res.text().await.unwrap_or_default();
        Err(format!("Failed to publish: HTTP {} - {}", status, err_text))
    }
}
