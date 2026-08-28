use miette::Result;
use pace_driver::CompilerSession;
use crate::utils::resolve_file;

pub fn execute(session: &CompilerSession, file: Option<String>, release: bool) -> Result<()> {
    let resolved_file = resolve_file(file)?;
    
    if let Ok(current_dir) = std::env::current_dir() {
        if let Ok(manifest) = pace_pkg::manifest::PaceToml::load_from_dir(&current_dir) {
            crate::utils::check_sdk_compatibility(&manifest)?;
        }
    }

    session.run_file(&resolved_file, release)?;
    Ok(())
}
