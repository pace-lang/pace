use miette::Result;
use pace_pkg::manifest::{Dependency, PaceToml};
use serde::Deserialize;

#[derive(Deserialize)]
struct RegistryResponse {
    latest_version: Option<String>,
}

pub fn execute(name: String, path: Option<String>, version: Option<String>) -> Result<()> {
    let current_dir = std::env::current_dir().map_err(|e| miette::miette!("Failed to get current dir: {}", e))?;
    
    // Determine the dependency type
    let dep = if let Some(p) = path {
        Dependency::Path { path: p }
    } else if let Some(v) = version {
        Dependency::Version(v)
    } else {
        println!("🔍 Looking up '{}' in the registry...", name);
        // Make sync HTTP request using ureq
        let registry_url = std::env::var("PACE_REGISTRY_URL").unwrap_or_else(|_| "https://registry.pace.dev".to_string());
        let url = format!("{}/api/packages/{}", registry_url, name);
        let resp = match ureq::get(&url).call() {
            Ok(r) => {
                if r.status() == 404 {
                    return Err(miette::miette!("Package '{}' not found in registry", name));
                } else if r.status() != 200 {
                    return Err(miette::miette!("Registry returned error status: {}", r.status()));
                }
                r
            },
            Err(e) => return Err(miette::miette!("Failed to connect to registry: {}", e)),
        };

        let parsed: RegistryResponse = resp.into_body().read_json().map_err(|e| miette::miette!("Failed to parse registry response: {}", e))?;
        let latest = parsed.latest_version.ok_or_else(|| miette::miette!("Package exists but has no published versions"))?;
        
        println!("📦 Found latest version: v{}", latest);
        Dependency::Version(latest)
    };

    println!("✍️  Adding '{}' to pace.toml...", name);
    PaceToml::add_dependency(&current_dir, &name, dep)
        .map_err(|e| miette::miette!("Failed to update pace.toml: {}", e))?;
        
    crate::commands::fetch::execute()?;
    Ok(())
}
