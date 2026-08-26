use miette::Result;
use pace_driver::CompilerSession;
use crate::utils::resolve_file;

pub fn execute(session: &CompilerSession, file: Option<String>, output_format: String) -> Result<()> {
    let resolved_file = resolve_file(file)?;
    
    if output_format != "json" {
        println!("Checking {}...", resolved_file);
    }
    
    let ast = session.check_file(&resolved_file)?;
    
    if output_format != "json" {
        println!("✅ Syntax OK");
        println!("{:#?}", ast);
    }
    
    Ok(())
}
