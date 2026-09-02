use miette::Result;

pub fn execute(is_pkg: bool) -> Result<()> {
    let current_dir =
        std::env::current_dir().map_err(|e| miette::miette!("Failed to get current dir: {}", e))?;
    let pace_toml = current_dir.join("pace.toml");

    if pace_toml.exists() {
        return Err(miette::miette!(
            "pace.toml already exists in the current directory"
        ));
    }

    let dir_name = current_dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let default_name = if dir_name.is_empty() {
        "my_project".to_string()
    } else {
        dir_name
    };

    let toml_content = format!(
        r#"[package]
name = "{0}"
version = "0.1.0"

[sdk]
pace = ">=0.1.0 <1.0.0"

[dependencies]

[dev-dependencies]
"#,
        default_name
    );
    std::fs::write(&pace_toml, toml_content)
        .map_err(|e| miette::miette!("Failed to write pace.toml: {}", e))?;

    let src_path = current_dir.join("src");
    let gitignore_path = current_dir.join(".gitignore");
    if !gitignore_path.exists() {
        let _ = std::fs::write(&gitignore_path, "target/\nbuild/\n.pace/\n");
    }
    if !src_path.exists() {
        std::fs::create_dir_all(&src_path)
            .map_err(|e| miette::miette!("Failed to create src directory: {}", e))?;
        if is_pkg {
            let lib_content = "func greet() {\n    print(\"Hello from Pace package!\");\n}\n";
            std::fs::write(src_path.join(format!("{}.pace", default_name)), lib_content)
                .map_err(|e| miette::miette!("Failed to write src/{}.pace: {}", default_name, e))?;
        } else {
            let main_content = "func main() {\n    print(\"⚡ Pace is ready.\");\n}\n";
            std::fs::write(src_path.join("main.pace"), main_content)
                .map_err(|e| miette::miette!("Failed to write src/main.pace: {}", e))?;
        }
    }

    println!(
        "✅ Initialized new Pace {} in current directory.",
        if is_pkg { "package" } else { "project" }
    );
    Ok(())
}
