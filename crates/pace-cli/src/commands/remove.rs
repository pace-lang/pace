use miette::Result;
use pace_pkg::manifest::PaceToml;

pub fn execute(name: String) -> Result<()> {
    let current_dir =
        std::env::current_dir().map_err(|e| miette::miette!("Failed to get current dir: {}", e))?;

    println!("🗑️  Removing '{}' from pace.toml...", name);
    PaceToml::remove_dependency(&current_dir, &name)
        .map_err(|e| miette::miette!("Failed to remove dependency: {}", e))?;

    crate::commands::fetch::execute()?;
    Ok(())
}
