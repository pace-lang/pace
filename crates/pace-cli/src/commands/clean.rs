use colored::Colorize;
use miette::Result;
use std::fs;

pub fn execute() -> Result<()> {
    let current_dir = std::env::current_dir()
        .map_err(|e| miette::miette!("Failed to get current directory: {}", e))?;

    let mut cleaned_items = 0;

    // 1. Remove build directory
    let build_dir = current_dir.join("build");
    if build_dir.exists() {
        if let Err(e) = fs::remove_dir_all(&build_dir) {
            eprintln!("{} Failed to remove build directory: {}", "⚠️".yellow(), e);
        } else {
            println!("{} Removed build directory", "🗑️".green());
            cleaned_items += 1;
        }
    }

    // 2. Remove .o files in the current directory
    if let Ok(entries) = fs::read_dir(&current_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("o") {
                if let Err(e) = fs::remove_file(&path) {
                    eprintln!(
                        "{} Failed to remove {}: {}",
                        "⚠️".yellow(),
                        path.display(),
                        e
                    );
                } else {
                    println!(
                        "{} Removed object file: {}",
                        "🗑️".green(),
                        path.file_name().unwrap_or_default().to_string_lossy()
                    );
                    cleaned_items += 1;
                }
            }
        }
    }

    if cleaned_items == 0 {
        println!("✨ Nothing to clean");
    } else {
        println!(
            "{} Successfully cleaned {} artifacts!",
            "✨".green(),
            cleaned_items
        );
    }

    Ok(())
}
