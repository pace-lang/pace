use miette::{miette, Result};

pub fn resolve_file(file: Option<String>) -> Result<String> {
    if let Some(f) = file {
        Ok(f)
    } else {
        if std::path::Path::new("pace.toml").exists() {
            if std::path::Path::new("src/main.pace").exists() {
                Ok("src/main.pace".to_string())
            } else if std::path::Path::new("src/lib.pace").exists() {
                Ok("src/lib.pace".to_string())
            } else {
                Err(miette!("Default entry point 'src/main.pace' or 'src/lib.pace' not found"))
            }
        } else {
            Err(miette!("No file specified and no pace.toml found in current directory"))
        }
    }
}
