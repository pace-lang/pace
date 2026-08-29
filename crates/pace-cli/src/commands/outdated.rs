use miette::Result;
use pace_pkg::manifest::{PaceToml, Dependency};
use pace_pkg::lockfile::PaceLock;
use pace_pkg::fetcher::Fetcher;

pub fn execute() -> Result<()> {
    let current_dir = std::env::current_dir().map_err(|e| miette::miette!("Failed to get current dir: {}", e))?;
    
    let manifest = PaceToml::load_from_dir(&current_dir)
        .map_err(|e| miette::miette!("Failed to load pace.toml: {}", e))?;
        
    let lock = PaceLock::load_from_dir(&current_dir)
        .unwrap_or_else(|_| None)
        .unwrap_or_default();
        
    println!("🔍 Checking for outdated dependencies...");
    
    let mut outdated_pkgs = Vec::new();
    
    for (pkg_name, dep) in &manifest.dependencies {
        if let Dependency::Version(_constraint) = dep {
            let current_version = lock.packages.get(pkg_name)
                .and_then(|p| p.version.clone())
                .unwrap_or_else(|| "none".to_string());
                
            let latest = match Fetcher::resolve_version(pkg_name, "*") {
                Ok((v, _)) => v,
                Err(_) => "unknown".to_string(),
            };

            if current_version != "none" && current_version != latest && latest != "unknown" {
                outdated_pkgs.push((pkg_name.clone(), current_version, latest));
            }
        }
    }
    
    if outdated_pkgs.is_empty() {
        println!("✨ All dependencies are up to date!");
    } else {
        println!("\n📦 Outdated packages found:\n");
        println!("{0: <20} | {1: <15} | {2: <15}", "Package", "Current", "Latest");
        println!("{0:-<20}-|-{0:-<15}-|-{0:-<15}-", "");
        for (pkg, current, latest) in outdated_pkgs {
            println!("{0: <20} | {1: <15} | {2: <15}", pkg, current, latest);
        }
        println!("\n💡 Run `pace update` to update all dependencies.");
    }
    
    Ok(())
}
