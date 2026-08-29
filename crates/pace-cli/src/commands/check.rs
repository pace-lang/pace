use crate::utils::resolve_file;
use miette::Result;
use pace_driver::CompilerSession;

pub fn execute(
    session: &CompilerSession,
    file: Option<String>,
    output_format: String,
) -> Result<()> {
    let resolved_file = resolve_file(file)?;

    if output_format != "json" {
        println!("Checking {}...", resolved_file);
    }

    session.check_file(&resolved_file)?;

    if output_format != "json" {
        println!("✅ Syntax OK");
    }

    Ok(())
}
