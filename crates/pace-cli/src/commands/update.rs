use miette::Result;
use pace_pkg::manifest::{PaceToml, Dependency};
use pace_pkg::fetcher::Fetcher;

pub fn execute() -> Result<()> {
    let current_dir = std::env::current_dir().map_err(|e| miette::miette!("Failed to get current dir: {}", e))?;
    
    // Check if pace.toml exists
    let manifest = PaceToml::load_from_dir(&current_dir)
        .map_err(|e| miette::miette!("Failed to load pace.toml: {}", e))?;
        
    println!("🔄 Updating dependencies...");
    
    // Update versions in pace.toml to absolute latest available on registry
    for (pkg_name, dep) in &manifest.dependencies {
        if let Dependency::Version(_constraint) = dep {
            if let Ok((latest, _)) = Fetcher::resolve_version(pkg_name, "*") {
                PaceToml::update_dependency_version(&current_dir, pkg_name, &latest)
                    .map_err(|e| miette::miette!("Failed to update pace.toml: {}", e))?;
            }
        }
    }
    
    // Delete the lockfile so that fetcher re-resolves everything
    let lock_path = current_dir.join("pace.lock");
    if lock_path.exists() {
        std::fs::remove_file(&lock_path)
            .map_err(|e| miette::miette!("Failed to remove pace.lock: {}", e))?;
    }
    
    crate::commands::fetch::execute()?;
    
    Ok(())
}
