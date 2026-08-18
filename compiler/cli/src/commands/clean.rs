use std::fs;
use std::process::exit;

use colored::Colorize;

use crate::utils::errors::print_global_error;
use crate::utils::workspace::find_package_root;

pub fn execute() {
    let root = match find_package_root() {
        Some(r) => r,
        None => {
            print_global_error(
                "Could not find `pace.toml` in current directory or any parent directory",
            );
            exit(1);
        }
    };

    let target_dir = root.join("target");
    if target_dir.exists() {
        if let Err(e) = fs::remove_dir_all(&target_dir) {
            print_global_error(&format!("Failed to remove target directory: {}", e));
            exit(1);
        }
        println!("{} target directory", "Cleaned".green().bold());
    } else {
        println!("{} target directory (already clean)", "Cleaned".green().bold());
    }
}
