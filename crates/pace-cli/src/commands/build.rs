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
    session.build_file(&resolved_file, &output)?;
    println!("✅ Build complete!");

    Ok(())
}
