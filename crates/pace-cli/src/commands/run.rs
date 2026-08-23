use miette::Result;
use pace_driver::CompilerSession;
use crate::utils::resolve_file;

pub fn execute(session: &CompilerSession, file: Option<String>) -> Result<()> {
    let resolved_file = resolve_file(file)?;
    session.run_file(&resolved_file)?;
    Ok(())
}
