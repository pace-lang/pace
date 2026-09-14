use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub mod cache;
pub mod resolve;
pub mod commands;

use std::collections::HashMap;

#[derive(Debug, Deserialize, Serialize)]
pub struct PaceToml {
    pub package: Package,
    #[serde(default)]
    pub environment: Option<Environment>,
    #[serde(default)]
    pub dependencies: HashMap<String, Dependency>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Environment {
    pub sdk: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Dependency {
    Version(String),
    Detailed {
        version: Option<String>,
        path: Option<String>,
    },
}

impl Dependency {
    pub fn version(&self) -> Option<&str> {
        match self {
            Dependency::Version(v) => Some(v),
            Dependency::Detailed { version, .. } => version.as_deref(),
        }
    }
    pub fn path(&self) -> Option<&str> {
        match self {
            Dependency::Version(_) => None,
            Dependency::Detailed { path, .. } => path.as_deref(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Package {
    pub name: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authors: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
}

fn validate_package_name(name: &str) -> Result<(), String> {
    if name.len() < 2 || name.len() > 64 {
        return Err("Package name must be between 2 and 64 characters long".into());
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_alphabetic() {
        return Err("Package name must start with a letter".into());
    }
    for c in chars {
        if !c.is_ascii_alphanumeric() && c != '_' {
            return Err("Package name can only contain letters, numbers, and underscores".into());
        }
    }
    Ok(())
}

pub fn scaffold_project(path: &str, is_lib: bool) -> Result<(), String> {
    let project_dir = Path::new(path);
    let pkg_name = project_dir
        .file_name()
        .ok_or("Invalid project path")?
        .to_str()
        .ok_or("Invalid UTF-8 in project path")?;

    validate_package_name(pkg_name)?;

    if project_dir.exists() {
        return Err(format!("Directory '{}' already exists", path));
    }

    fs::create_dir_all(project_dir.join("src"))
        .map_err(|e| format!("Failed to create src directory: {}", e))?;

    let toml_content = if is_lib {
        format!(
            "[package]\nname = \"{}\"\nversion = \"0.1.0\"\ndescription = \"A pace package\"\nlicense = \"MIT\"\nauthors = [\"Your Name <you@example.com>\"]\nrepository = \"https://github.com/your/repo\"\n\n[environment]\nsdk = \">=0.1.0\"\n",
            pkg_name
        )
    } else {
        format!("[package]\nname = \"{}\"\nversion = \"0.1.0\"\n\n[environment]\nsdk = \">=0.1.0\"\n", pkg_name)
    };

    fs::write(project_dir.join("pace.toml"), toml_content)
        .map_err(|e| format!("Failed to write pace.toml: {}", e))?;

    let gitignore_content = "/build\n/pace.lock\n/.pace\n";
    fs::write(project_dir.join(".gitignore"), gitignore_content)
        .map_err(|e| format!("Failed to write .gitignore: {}", e))?;

    let readme_content = format!("# {}\n\nWelcome to your new Pace project!", pkg_name);
    fs::write(project_dir.join("README.md"), readme_content)
        .map_err(|e| format!("Failed to write README.md: {}", e))?;

    if is_lib {
        fs::write(
            project_dir.join("LICENSE"),
            "MIT License\n\nCopyright (c) 2026 ...\n",
        )
        .map_err(|e| format!("Failed to write LICENSE: {}", e))?;
        fs::write(
            project_dir.join("CHANGELOG.md"),
            "# Changelog\n\n## [0.1.0] - Initial release\n",
        )
        .map_err(|e| format!("Failed to write CHANGELOG.md: {}", e))?;

        let lib_content = "fn hello() {\n    println(\"⚡ Hello from your Pace library!\")\n}\n";
        fs::write(project_dir.join("src/lib.pace"), lib_content)
            .map_err(|e| format!("Failed to write src/lib.pace: {}", e))?;
    } else {
        let main_content =
            "fn main() {\n    println(\"⚡ Pace is ready. Build something fast.\")\n}\n";
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
    let content =
        fs::read_to_string(path).map_err(|e| format!("Failed to read pace.toml: {}", e))?;
    toml::from_str(&content).map_err(|e| format!("Failed to parse pace.toml: {}", e))
}
