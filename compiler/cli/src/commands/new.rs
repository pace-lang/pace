use std::fs;
use std::path::Path;
use std::process::exit;

use crate::utils::errors::print_global_error;

fn is_kebab_case(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }

    let chars: Vec<char> = s.chars().collect();

    if !chars[0].is_ascii_lowercase() {
        return false;
    }

    if chars.last() == Some(&'-') {
        return false;
    }

    let mut last_was_hyphen = false;
    for &c in &chars {
        if !c.is_ascii_lowercase() && !c.is_ascii_digit() && c != '-' {
            return false;
        }
        if c == '-' {
            if last_was_hyphen {
                return false;
            }
            last_was_hyphen = true;
        } else {
            last_was_hyphen = false;
        }
    }

    true
}

pub fn execute(path: &Path, name: &str) {
    if !is_kebab_case(name) {
        eprintln!("error: invalid project name `{}`", name);
        eprintln!("help: project names must contain only lowercase letters, numbers, and `-`");
        eprintln!("help: example: `my-app`");
        exit(1);
    }
    if path.exists() {
        if path
            .read_dir()
            .map(|mut i| i.next().is_some())
            .unwrap_or(false)
            && path.join("pace.toml").exists()
        {
            print_global_error(&format!(
                "Directory {:?} already contains a pace.toml",
                path
            ));
            exit(1);
        }
    } else {
        if let Err(e) = fs::create_dir_all(path) {
            crate::utils::errors::print_global_error(&format!(
                "Failed to create directory {:?}: {}",
                path, e
            ));
            exit(1);
        }
    }

    if let Err(e) = fs::create_dir_all(path.join("src")) {
        crate::utils::errors::print_global_error(&format!("Failed to create src directory: {}", e));
        exit(1);
    }
    let toml = format!(
        r#"[package]
name = "{}"
version = "0.1.0"

[dependencies]
"#,
        name
    );
    if let Err(e) = fs::write(path.join("pace.toml"), toml) {
        crate::utils::errors::print_global_error(&format!("Failed to write pace.toml: {}", e));
        exit(1);
    }

    let main_pace = r#"func main() {
    print("✨ Welcome to Pace — let's make something great.");
}
"#;
    if let Err(e) = fs::write(path.join("src").join("main.pace"), main_pace) {
        crate::utils::errors::print_global_error(&format!("Failed to write main.pace: {}", e));
        exit(1);
    }

    let gitignore = r#"/target
"#;
    if let Err(e) = fs::write(path.join(".gitignore"), gitignore) {
        crate::utils::errors::print_global_error(&format!("Failed to write .gitignore: {}", e));
        exit(1);
    }

    let readme = format!(
        r#"# {}

A Pace language project.

## Getting Started

To run your project directly, use:
```sh
pace run
```

To build a standalone executable in the `target/` directory, use:
```sh
pace build
```
"#,
        name
    );
    if let Err(e) = fs::write(path.join("README.md"), readme) {
        crate::utils::errors::print_global_error(&format!("Failed to write README.md: {}", e));
        exit(1);
    }
    use colored::Colorize;

    println!(" ");
    println!("Created {} project successfully\n", "pace".green().bold());
    println!("{}", "# To get started, run:\n".bright_black());
    println!("  cd {}", name.green().bold());
    println!("  pace run");
    println!(" ")
}
