use miette::Result;
use colored::Colorize;
use pace_pkg::manifest::{PaceToml, Dependency};
use pace_pkg::fetcher::Fetcher;

pub fn execute() -> Result<()> {
    let current_dir = std::env::current_dir().map_err(|e| miette::miette!("Failed to get current dir: {}", e))?;
    
    let manifest = PaceToml::load_from_dir(&current_dir)
        .map_err(|e| miette::miette!("Failed to load pace.toml: {}", e))?;
        
    println!("{}", "🔄 Updating packages...".cyan());
    
    let mut updated = false;

    // Update versions in pace.toml if there's a newer version available
    for (pkg_name, dep) in &manifest.dependencies {
        if let Dependency::Version(constraint) = dep {
            if let Ok((latest, _)) = Fetcher::resolve_version(pkg_name, "*") {
                if latest != *constraint {
                    println!("DEBUG: Updating {} from {} to {}", pkg_name, constraint, latest);
                    PaceToml::update_dependency_version(&current_dir, pkg_name, &latest)
                        .map_err(|e| miette::miette!("Failed to update pace.toml: {}", e))?;
                    updated = true;
                }
            }
        }
    }
    
    if updated {
        println!("{}", "📝 Updated pace.toml with latest matching versions.".green());
    }
    
    // Delete the lockfile so that fetcher re-resolves everything to latest compatible versions
    let lock_path = current_dir.join("pace.lock");
    if lock_path.exists() {
        std::fs::remove_file(&lock_path)
            .map_err(|e| miette::miette!("Failed to remove pace.lock: {}", e))?;
    }
    
    crate::commands::fetch::execute()?;
    
    Ok(())
}

