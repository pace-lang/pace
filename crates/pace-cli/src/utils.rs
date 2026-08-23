use miette::{miette, Result};

pub fn resolve_file(file: Option<String>) -> Result<String> {
    if let Some(f) = file {
        Ok(f)
    } else {
        if std::path::Path::new("pace.toml").exists() {
            let default_path = "src/main.pace";
            if std::path::Path::new(default_path).exists() {
                Ok(default_path.to_string())
            } else {
                Err(miette!("Default entry point '{}' not found", default_path))
            }
        } else {
            Err(miette!("No file specified and no pace.toml found in current directory"))
        }
    }
}
