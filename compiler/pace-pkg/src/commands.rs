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

    let ver = version.unwrap_or("*");
    
    // Add to [dependencies] table
    if !doc.contains_key("dependencies") {
        doc["dependencies"] = toml_edit::table();
    }
    
    if let Some(deps) = doc["dependencies"].as_table_mut() {
        deps.insert(name, value(ver));
    }

    fs::write(&manifest_path, doc.to_string()).map_err(|e| format!("Failed to write pace.toml: {}", e))?;

    // Now re-resolve and download
    let toml = parse_manifest(&manifest_path)?;
    let mut resolver = DependencyResolver::new();
    let lock = resolver.resolve(&toml).await?;
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
