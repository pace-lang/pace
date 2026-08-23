use miette::Result;

pub fn execute(name: String) -> Result<()> {
    // Validation
    if name.len() > 15 {
        return Err(miette::miette!("Project name must be 15 characters or less"));
    }

    if name.is_empty() {
        return Err(miette::miette!("Project name cannot be empty"));
    }

    let first_char = name.chars().next().unwrap();
    if !first_char.is_ascii_alphabetic() {
        return Err(miette::miette!("Project name must start with a letter"));
    }

    for c in name.chars() {
        if !c.is_ascii_alphanumeric() && c != '_' {
            return Err(miette::miette!("Project name can only contain alphanumeric characters and underscores"));
        }
    }

    println!("Creating new Pace project: {}", name);
    
    let project_path = std::path::Path::new(&name);
    let src_path = project_path.join("src");
    
    std::fs::create_dir_all(&src_path)
        .map_err(|e| miette::miette!("Failed to create project directories: {}", e))?;
        
    let toml_content = format!("[package]\nname = \"{}\"\nversion = \"0.1.0\"\n", name);
    std::fs::write(project_path.join("pace.toml"), toml_content)
        .map_err(|e| miette::miette!("Failed to write pace.toml: {}", e))?;
        
    std::fs::write(project_path.join("pace.lock"), "")
        .map_err(|e| miette::miette!("Failed to write pace.lock: {}", e))?;
        
    std::fs::write(project_path.join(".gitignore"), "target/\nbuild/\n")
        .map_err(|e| miette::miette!("Failed to write .gitignore: {}", e))?;
        
    let readme_content = format!("# {}\n\nA Pace project.\n", name);
    std::fs::write(project_path.join("README.md"), readme_content)
        .map_err(|e| miette::miette!("Failed to write README.md: {}", e))?;
        
    let main_content = "func main() {\n    print(\"⚡ Pace is ready. Build something fast.\");\n}\n";
    std::fs::write(src_path.join("main.pace"), main_content)
        .map_err(|e| miette::miette!("Failed to write src/main.pace: {}", e))?;
        
    println!("✅ Created project '{}'", name);
    
    Ok(())
}
