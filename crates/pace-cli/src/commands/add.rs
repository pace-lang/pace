use miette::Result;
use pace_pkg::fetcher::Fetcher;
use pace_pkg::manifest::{Dependency, PaceToml};

pub fn execute(name: String, path: Option<String>, version: Option<String>) -> Result<()> {
    let current_dir = std::env::current_dir().map_err(|e| miette::miette!("Failed to get current dir: {}", e))?;
    
    // Determine the dependency type
    let dep = if let Some(p) = path {
        Dependency::Path { path: p }
    } else {
        let constraint = version.unwrap_or_else(|| "*".to_string());
        println!("🔍 Looking up '{}' in the registry...", name);
        let (exact_version, _) = Fetcher::resolve_version(&name, &constraint)?;
        println!("📦 Found version: v{}", exact_version);
        Dependency::Version(exact_version)
    };

    println!("✍️  Adding '{}' to pace.toml...", name);
    PaceToml::add_dependency(&current_dir, &name, dep)
        .map_err(|e| miette::miette!("Failed to update pace.toml: {}", e))?;
        
    crate::commands::fetch::execute()?;
    Ok(())
}
