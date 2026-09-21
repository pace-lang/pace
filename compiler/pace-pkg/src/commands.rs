use crate::{cache::CacheManager, find_manifest, parse_manifest, resolve::DependencyResolver};
use std::env;
use std::fs;
use toml_edit::{DocumentMut, value};

pub async fn add_dependency(name: &str, version: Option<&str>, dev: bool) -> Result<(), String> {
    let current_dir = env::current_dir().map_err(|_| "Failed to get current directory")?;
    let manifest_path = find_manifest(&current_dir)
        .ok_or("No pace.toml found in this directory or any parent directory")?;

    let content = fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Failed to read pace.toml: {}", e))?;
    let mut doc = content
        .parse::<DocumentMut>()
        .map_err(|e| format!("Failed to parse pace.toml: {}", e))?;

    let ver = if let Some(v) = version {
        v.to_string()
    } else {
        println!("Fetching latest version of {}...", name);
        let resolver = DependencyResolver::new();
        match resolver.get_package_info(name).await {
            Ok(info) => {
                if let Some(lv) = info.latest_version {
                    format!("^{}", lv)
                } else if let Some(v_info) = info.version_info.last() {
                    format!("^{}", v_info.version)
                } else {
                    "*".to_string()
                }
            }
            Err(e) => return Err(e),
        }
    };

    let table_name = if dev {
        "dev-dependencies"
    } else {
        "dependencies"
    };

    if !doc.contains_key(table_name) {
        doc[table_name] = toml_edit::table();
    }

    if let Some(deps) = doc[table_name].as_table_mut() {
        deps.insert(name, value(&ver));
    }

    let doc_str = doc.to_string();
    let toml: crate::PaceToml = toml::from_str(&doc_str)
        .map_err(|e| format!("Failed to parse updated pace.toml: {}", e))?;

    let mut resolver = DependencyResolver::new();
    let lock = match resolver.resolve(&toml).await {
        Ok(l) => l,
        Err(e) => {
            return Err(format!(
                "Dependency resolution failed with version {}: {}",
                ver, e
            ));
        }
    };

    // If resolution succeeds, write pace.toml and pace.lock
    fs::write(&manifest_path, doc_str).map_err(|e| format!("Failed to write pace.toml: {}", e))?;
    let root = manifest_path.parent().ok_or("Invalid manifest path")?;
    resolver.write_lockfile(&root.join("pace.lock"), &lock)?;

    let cache = CacheManager::new();
    for (pkg_name, pkg_info) in &lock.packages {
        if let Some(checksum) = &pkg_info.checksum {
            cache
                .download_and_extract(pkg_name, &pkg_info.version, checksum)
                .await?;
        }
    }

    println!("Added {} version {}", name, ver);
    Ok(())
}

pub async fn remove_dependency(name: &str) -> Result<(), String> {
    let current_dir = env::current_dir().map_err(|_| "Failed to get current directory")?;
    let manifest_path = find_manifest(&current_dir)
        .ok_or("No pace.toml found in this directory or any parent directory")?;

    let content = fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Failed to read pace.toml: {}", e))?;
    let mut doc = content
        .parse::<DocumentMut>()
        .map_err(|e| format!("Failed to parse pace.toml: {}", e))?;

    let mut removed = false;

    if let Some(deps) = doc.get_mut("dependencies").and_then(|i| i.as_table_mut())
        && deps.remove(name).is_some()
    {
        removed = true;
    }

    if let Some(dev_deps) = doc
        .get_mut("dev-dependencies")
        .and_then(|i| i.as_table_mut())
        && dev_deps.remove(name).is_some()
    {
        removed = true;
    }

    if !removed {
        return Err(format!("Package '{}' is not a dependency", name));
    }

    let doc_str = doc.to_string();
    let toml: crate::PaceToml = toml::from_str(&doc_str)
        .map_err(|e| format!("Failed to parse updated pace.toml: {}", e))?;

    let mut resolver = DependencyResolver::new();
    let lock = match resolver.resolve(&toml).await {
        Ok(l) => l,
        Err(e) => return Err(format!("Dependency resolution failed after removal: {}", e)),
    };

    fs::write(&manifest_path, doc_str).map_err(|e| format!("Failed to write pace.toml: {}", e))?;
    let root = manifest_path.parent().ok_or("Invalid manifest path")?;
    resolver.write_lockfile(&root.join("pace.lock"), &lock)?;

    println!("Removed {}", name);
    Ok(())
}

pub fn clean(cache: bool) -> Result<(), String> {
    if cache && let Some(home) = dirs::home_dir() {
        let cache_dir = home.join(".pace").join("cache");
        if cache_dir.exists() {
            fs::remove_dir_all(&cache_dir)
                .map_err(|e| format!("Failed to clean cache directory: {}", e))?;
            println!("Cleaned global cache at {}", cache_dir.display());
        } else {
            println!("Global cache is already empty.");
        }
    }

    let current_dir = env::current_dir().map_err(|_| "Failed to get current directory")?;
    if let Some(manifest_path) = find_manifest(&current_dir) {
        let build_dir = manifest_path
            .parent()
            .ok_or("Invalid manifest path")?
            .join("build");
        if build_dir.exists() {
            fs::remove_dir_all(&build_dir)
                .map_err(|e| format!("Failed to clean build directory: {}", e))?;
            println!("Cleaned build directory at {}", build_dir.display());
        } else {
            println!("Build directory is already empty.");
        }
    } else {
        if !cache {
            return Err("No pace.toml found in this directory or any parent directory".to_string());
        }
    }

    Ok(())
}

pub async fn update_dependencies(latest: bool) -> Result<(), String> {
    let current_dir = env::current_dir().map_err(|_| "Failed to get current directory")?;
    let manifest_path = find_manifest(&current_dir)
        .ok_or("No pace.toml found in this directory or any parent directory")?;

    let mut toml = parse_manifest(&manifest_path)?;
    let mut resolver = DependencyResolver::new();

    if latest {
        // If latest, we strip version constraints for registry dependencies
        for dep in toml.dependencies.values_mut() {
            if let crate::Dependency::Version(_) = dep {
                *dep = crate::Dependency::Version("*".to_string());
            } else if let crate::Dependency::Detailed { version, path } = dep
                && path.is_none()
            {
                *version = Some("*".to_string());
            }
        }
        for dep in toml.dev_dependencies.values_mut() {
            if let crate::Dependency::Version(_) = dep {
                *dep = crate::Dependency::Version("*".to_string());
            } else if let crate::Dependency::Detailed { version, path } = dep
                && path.is_none()
            {
                *version = Some("*".to_string());
            }
        }
    }

    let root = manifest_path.parent().ok_or("Invalid manifest path")?;
    let lockfile_path = root.join("pace.lock");

    let old_lock = if lockfile_path.exists() {
        DependencyResolver::read_lockfile(&lockfile_path).ok()
    } else {
        None
    };

    if let Some(ref lock) = old_lock {
        resolver.load_lock(lock.clone());
    }

    let lock = resolver.resolve(&toml).await?;
    resolver.write_lockfile(&lockfile_path, &lock)?;

    if latest {
        let content = fs::read_to_string(&manifest_path)
            .map_err(|e| format!("Failed to read pace.toml: {}", e))?;
        let mut doc = content
            .parse::<DocumentMut>()
            .map_err(|e| format!("Failed to parse pace.toml: {}", e))?;

        if let Some(deps) = doc["dependencies"].as_table_mut() {
            for (pkg_name, pkg_info) in &lock.packages {
                if let Some(source) = &pkg_info.source
                    && source.starts_with("local+")
                {
                    continue;
                }
                if deps.contains_key(pkg_name) {
                    deps.insert(pkg_name, value(format!("^{}", pkg_info.version)));
                }
            }
        }
        if let Some(dev_deps) = doc["dev-dependencies"].as_table_mut() {
            for (pkg_name, pkg_info) in &lock.packages {
                if let Some(source) = &pkg_info.source
                    && source.starts_with("local+")
                {
                    continue;
                }
                if dev_deps.contains_key(pkg_name) {
                    dev_deps.insert(pkg_name, value(format!("^{}", pkg_info.version)));
                }
            }
        }
        fs::write(&manifest_path, doc.to_string())
            .map_err(|e| format!("Failed to write pace.toml: {}", e))?;
    }

    let cache = CacheManager::new();
    for (pkg_name, pkg_info) in &lock.packages {
        if let Some(checksum) = &pkg_info.checksum {
            cache
                .download_and_extract(pkg_name, &pkg_info.version, checksum)
                .await?;
        }
    }

    let mut changed = false;
    if let Some(old_lock) = old_lock {
        for (name, new_pkg) in &lock.packages {
            if let Some(old_pkg) = old_lock.packages.get(name) {
                if new_pkg.version != old_pkg.version {
                    println!(
                        "    Updating {} v{} -> v{}",
                        name, old_pkg.version, new_pkg.version
                    );
                    changed = true;
                }
            } else {
                println!("      Adding {} v{}", name, new_pkg.version);
                changed = true;
            }
        }
        for name in old_lock.packages.keys() {
            if !lock.packages.contains_key(name) {
                println!("    Removing {}", name);
                changed = true;
            }
        }
    } else {
        for (name, new_pkg) in &lock.packages {
            println!("      Adding {} v{}", name, new_pkg.version);
            changed = true;
        }
    }

    if !changed {
        println!("Dependencies are up to date.");
    }

    Ok(())
}

pub async fn list_outdated() -> Result<(), String> {
    let current_dir = env::current_dir().map_err(|_| "Failed to get current directory")?;
    let manifest_path = find_manifest(&current_dir)
        .ok_or("No pace.toml found in this directory or any parent directory")?;

    let root = manifest_path.parent().ok_or("Invalid manifest path")?;
    let lockfile_path = root.join("pace.lock");
    if !lockfile_path.exists() {
        println!("No pace.lock found. Run `pace build` or `pace update` first.");
        return Ok(());
    }

    let toml = parse_manifest(&manifest_path)?;
    let lock = DependencyResolver::read_lockfile(&lockfile_path)?;
    let resolver = DependencyResolver::new();

    println!(
        "{:<20} {:<15} {:<15} {:<15}",
        "Package", "Current", "Update", "Latest"
    );
    println!("{:-<20} {:-<15} {:-<15} {:-<15}", "", "", "", "");

    let mut found_outdated = false;

    for (pkg_name, locked_pkg) in &lock.packages {
        // Skip local path dependencies
        if let Some(src) = &locked_pkg.source
            && src.starts_with("local+")
        {
            continue;
        }

        let required = if let Some(dep) = toml.dependencies.get(pkg_name) {
            dep.version().unwrap_or("*").to_string()
        } else if let Some(dep) = toml.dev_dependencies.get(pkg_name) {
            dep.version().unwrap_or("*").to_string()
        } else {
            // Transitive dependency
            "".to_string()
        };

        if let Ok(info) = resolver.get_package_info(pkg_name).await {
            let mut highest_ver = semver::Version::parse("0.0.0").unwrap();
            let mut highest_compatible_ver = semver::Version::parse("0.0.0").unwrap();
            let req_parsed = semver::VersionReq::parse(&required).ok();

            for v_info in &info.version_info {
                if let Ok(ver) = semver::Version::parse(&v_info.version) {
                    if ver > highest_ver {
                        highest_ver = ver.clone();
                    }
                    if let Some(req) = &req_parsed
                        && req.matches(&ver)
                        && ver > highest_compatible_ver
                    {
                        highest_compatible_ver = ver.clone();
                    }
                }
            }

            let locked_ver = semver::Version::parse(&locked_pkg.version)
                .unwrap_or_else(|_| semver::Version::parse("0.0.0").unwrap());

            let update_str = if required.is_empty() {
                "".to_string()
            } else if highest_compatible_ver > semver::Version::parse("0.0.0").unwrap() {
                highest_compatible_ver.to_string()
            } else {
                required.clone()
            };

            if highest_ver > locked_ver {
                found_outdated = true;
                println!(
                    "{:<20} {:<15} {:<15} {:<15}",
                    pkg_name,
                    locked_pkg.version,
                    update_str,
                    highest_ver.to_string()
                );
            }
        }
    }

    if !found_outdated {
        println!("Everything is up to date.");
    }

    Ok(())
}

use flate2::Compression;
use flate2::write::GzEncoder;
use reqwest::multipart;

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

    let root = manifest_path.parent().ok_or("Invalid manifest path")?;
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
            let doc = content
                .parse::<toml_edit::DocumentMut>()
                .unwrap_or_default();
            if let Some(t) = doc.get("token").and_then(|t| t.as_str()) {
                return Ok(t.to_string());
            }
        }
        Err("PACE_TOKEN environment variable is not set and no credentials found")
    })?;

    // Create a temporary file for the tarball
    let tarball_path = root.join("target").join(format!(
        "{}-{}.tar.gz",
        toml.package.name, toml.package.version
    ));
    if let Some(p) = tarball_path.parent() {
        fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }

    let tar_gz =
        fs::File::create(&tarball_path).map_err(|e| format!("Failed to create tarball: {}", e))?;
    let enc = GzEncoder::new(tar_gz, Compression::default());
    let mut tar = tar::Builder::new(enc);

    let walker = walkdir::WalkDir::new(root).into_iter();
    for entry in walker.filter_entry(|e| {
        let name = e.file_name().to_string_lossy();
        name != "target"
            && name != "build"
            && name != ".git"
            && name != "pace.lock"
            && name != ".pace"
    }) {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let relative = path.strip_prefix(root).map_err(|e| e.to_string())?;

        if path.is_file() {
            tar.append_path_with_name(path, relative)
                .map_err(|e| format!("Failed to add to tarball: {}", e))?;
        }
    }
    let mut enc = tar
        .into_inner()
        .map_err(|e| format!("Failed to finish tar: {}", e))?;
    enc.try_finish()
        .map_err(|e| format!("Failed to finish gzip: {}", e))?;
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
        .part(
            "tarball",
            multipart::Part::bytes(tarball_bytes)
                .file_name("package.tar.gz")
                .mime_str("application/gzip")
                .unwrap(),
        );

    let client = reqwest::Client::new();
    let res = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", token))
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("Failed to send publish request: {}", e))?;

    if res.status().is_success() {
        println!(
            "Successfully published {} v{}",
            toml.package.name, toml.package.version
        );
        Ok(())
    } else {
        let status = res.status();
        let err_text = res.text().await.unwrap_or_default();
        Err(format!("Failed to publish: HTTP {} - {}", status, err_text))
    }
}
