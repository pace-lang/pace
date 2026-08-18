use std::process::{Command, exit};

use crate::commands::build;
use crate::utils::errors::print_global_error;

pub fn execute(file: Option<&str>, release: bool) {
    let out_file = build::execute(file, release);
    let bin_path = out_file.to_str().unwrap();
    let status = match Command::new(bin_path).status() {
        Ok(s) => s,
        Err(e) => {
            print_global_error(&format!("Failed to execute process `{}`: {}", bin_path, e));
            exit(1);
        }
    };
    exit(status.code().unwrap_or(1));
}
