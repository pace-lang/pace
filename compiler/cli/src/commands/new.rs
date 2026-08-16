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
        fs::create_dir_all(path).unwrap();
    }

    fs::create_dir_all(path.join("src")).unwrap();
    let toml = format!(
        r#"[package]
name = "{}"
version = "0.1.0"

[dependencies]
"#,
        name
    );
    fs::write(path.join("pace.toml"), toml).unwrap();

    let main_pace = r#"func main() {
    print("✨ Welcome to Pace — let's make something great.");
}
"#;
    fs::write(path.join("src").join("main.pace"), main_pace).unwrap();
    use colored::Colorize;
    
    println!(" ");
    println!(
        "Created {} project successfully\n",
        "pace".green().bold()
    );
    println!("{}", "# To get started, run:\n".bright_black());
    println!("  cd {}", name.green().bold());
    println!("  pace run");
    println!(" ")
}
