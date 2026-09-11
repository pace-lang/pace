use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Serialize)]
pub struct PaceToml {
    pub package: Package,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Package {
    pub name: String,
    pub version: String,
}

pub fn scaffold_project(name: &str, is_lib: bool) -> Result<(), String> {
    let project_dir = Path::new(name);
    if project_dir.exists() {
        return Err(format!("Directory '{}' already exists", name));
    }

    fs::create_dir_all(project_dir.join("src"))
        .map_err(|e| format!("Failed to create src directory: {}", e))?;

    let toml_content = format!(
        "[package]\nname = \"{}\"\nversion = \"0.1.0\"\n",
        name
    );
    fs::write(project_dir.join("pace.toml"), toml_content)
        .map_err(|e| format!("Failed to write pace.toml: {}", e))?;

    if is_lib {
        let lib_content = "fn hello() {\n    print(42)\n}\n";
        fs::write(project_dir.join("src/lib.pace"), lib_content)
            .map_err(|e| format!("Failed to write src/lib.pace: {}", e))?;
    } else {
        let main_content = "fn main() {\n    print(42)\n}\n";
        fs::write(project_dir.join("src/main.pace"), main_content)
            .map_err(|e| format!("Failed to write src/main.pace: {}", e))?;
    }

    Ok(())
}

pub fn find_manifest(current_dir: &Path) -> Option<PathBuf> {
    let mut dir = current_dir.to_path_buf();
    loop {
        let manifest_path = dir.join("pace.toml");
        if manifest_path.exists() {
            return Some(manifest_path);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

pub fn parse_manifest(path: &Path) -> Result<PaceToml, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read pace.toml: {}", e))?;
    toml::from_str(&content)
        .map_err(|e| format!("Failed to parse pace.toml: {}", e))
}
