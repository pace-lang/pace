use crate::utils::resolve_file;
use miette::Result;
use pace_driver::Compiler;

pub fn execute(session: &Compiler, file: Option<String>) -> Result<()> {
    let resolved_file = resolve_file(file)?;

    if let Ok(current_dir) = std::env::current_dir()
        && let Ok(manifest) = pace_pkg::manifest::PaceToml::load_from_dir(&current_dir)
    {
        crate::utils::check_sdk_compatibility(&manifest)?;
    }

    session.run_file(&resolved_file)?;
    Ok(())
}
