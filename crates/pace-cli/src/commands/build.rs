use miette::Result;
use pace_driver::CompilerSession;
use crate::utils::resolve_file;

pub fn execute(session: &CompilerSession, file: Option<String>, release: bool) -> Result<()> {
    let resolved_file = resolve_file(file)?;
    let output_name = std::path::Path::new(&resolved_file)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    
    let build_dir = std::path::Path::new("build");
    if !build_dir.exists() {
        std::fs::create_dir_all(build_dir)
            .map_err(|e| miette::miette!("Failed to create build directory: {}", e))?;
    }
    
    let output = if output_name.is_empty() { 
        build_dir.join("output").to_string_lossy().into_owned() 
    } else { 
        build_dir.join(output_name).to_string_lossy().into_owned() 
    };
    
    println!("Building {} to ./{}...", resolved_file, output);
    session.build_file(&resolved_file, &output, release)?;
    println!("✅ Build complete!");
    
    Ok(())
}
