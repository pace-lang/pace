use miette::Result;

pub fn execute(name: String, is_pkg: bool) -> Result<()> {
    // Validation
    if name.len() > 15 {
        return Err(miette::miette!(
            "Project name must be 15 characters or less"
        ));
    }

    if name.is_empty() {
        return Err(miette::miette!("Project name cannot be empty"));
    }

    let first_char = name.chars().next().unwrap();
    if !first_char.is_ascii_alphabetic() {
        return Err(miette::miette!("Project name must start with a letter"));
    }

    for c in name.chars() {
        if !c.is_ascii_alphanumeric() && c != '-' && c != '_' {
            return Err(miette::miette!(
                "Project name can only contain alphanumeric characters, hyphens, and underscores"
            ));
        }
    }

    println!("Creating new Pace project: {}", name);

    let project_path = std::path::Path::new(&name);
    let src_path = project_path.join("src");

    std::fs::create_dir_all(&src_path)
        .map_err(|e| miette::miette!("Failed to create project directories: {}", e))?;

    let toml_content = format!(
        r#"[package]
name = "{0}"
version = "0.1.0"
description = "A new Pace package"
license = "MIT"
authors = ["Your Name <you@example.com>"]
repository = "https://github.com/user/{0}"

[sdk]
pace = ">=0.1.0 <1.0.0"

[dependencies]

[dev-dependencies]
"#,
        name
    );
    std::fs::write(project_path.join("pace.toml"), toml_content)
        .map_err(|e| miette::miette!("Failed to write pace.toml: {}", e))?;

    std::fs::write(project_path.join("pace.lock"), "")
        .map_err(|e| miette::miette!("Failed to write pace.lock: {}", e))?;

    std::fs::write(project_path.join(".gitignore"), "target/\nbuild/\n.pace/\n")
        .map_err(|e| miette::miette!("Failed to write .gitignore: {}", e))?;

    let readme_content = format!(
        "# {}\n\nA Pace {}.\n",
        name,
        if is_pkg { "package" } else { "project" }
    );
    std::fs::write(project_path.join("README.md"), readme_content)
        .map_err(|e| miette::miette!("Failed to write README.md: {}", e))?;

    if is_pkg {
        let lib_content = "func greet() {\n    print(\"Hello from Pace package!\");\n}\n";
        std::fs::write(src_path.join(format!("{}.pace", name)), lib_content)
            .map_err(|e| miette::miette!("Failed to write src/{}.pace: {}", name, e))?;

        std::fs::write(project_path.join("LICENSE"), "MIT License\n")
            .map_err(|e| miette::miette!("Failed to write LICENSE: {}", e))?;
        std::fs::write(
            project_path.join("CHANGELOG.md"),
            "# Changelog\n\n## 0.1.0\n- Initial release\n",
        )
        .map_err(|e| miette::miette!("Failed to write CHANGELOG.md: {}", e))?;

        println!("✅ Created package '{}'", name);
    } else {
        let main_content =
            "func main() {\n    print(\"⚡ Pace is ready. Build something fast.\");\n}\n";
        std::fs::write(src_path.join("main.pace"), main_content)
            .map_err(|e| miette::miette!("Failed to write src/main.pace: {}", e))?;
        println!("✅ Created project '{}'", name);
    }

    // Create tests and examples
    let tests_path = project_path.join("tests");
    let examples_path = project_path.join("examples");
    std::fs::create_dir_all(&tests_path)
        .map_err(|e| miette::miette!("Failed to create tests directory: {}", e))?;
    std::fs::create_dir_all(&examples_path)
        .map_err(|e| miette::miette!("Failed to create examples directory: {}", e))?;

    std::fs::write(
        tests_path.join("basic_test.pace"),
        "func test_basic() {\n    print(\"Basic test passed\");\n}\n",
    )
    .map_err(|e| miette::miette!("Failed to write tests/basic_test.pace: {}", e))?;

    // Create examples as a full project
    let demo_path = examples_path;
    std::fs::create_dir_all(demo_path.join("src"))
        .map_err(|e| miette::miette!("Failed to create examples/src: {}", e))?;

    let demo_toml = format!(
        r#"[package]
name = "demo"
version = "0.1.0"
description = "Example demo for {0}"

[sdk]
pace = ">=0.1.0 <1.0.0"

[dependencies]
{0} = {{ path = ".." }}
"#,
        name
    );
    std::fs::write(demo_path.join("pace.toml"), demo_toml)
        .map_err(|e| miette::miette!("Failed to write examples/pace.toml: {}", e))?;

    std::fs::write(
        demo_path.join("src").join("main.pace"),
        format!(
            "import \"package:{}\";\n\nfunc main() {{\n    print(\"Running demo\");\n}}\n",
            name
        ),
    )
    .map_err(|e| miette::miette!("Failed to write examples/src/main.pace: {}", e))?;

    Ok(())
}
