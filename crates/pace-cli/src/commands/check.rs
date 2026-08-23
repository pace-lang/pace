use miette::Result;
use pace_driver::CompilerSession;
use crate::utils::resolve_file;

pub fn execute(session: &CompilerSession, file: Option<String>) -> Result<()> {
    let resolved_file = resolve_file(file)?;
    println!("Checking {}...", resolved_file);
    let ast = session.check_file(&resolved_file)?;
    println!("✅ Syntax OK");
    println!("{:#?}", ast);
    
    Ok(())
}
