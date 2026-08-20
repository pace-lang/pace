use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;

use codegen::CraneliftGenerator;
use linker::Linker;

use crate::utils::compiler::compile_to_mir;
use crate::utils::errors::print_global_error;
use crate::utils::workspace::find_package_root;

pub fn execute(override_file: Option<&str>, release: bool) -> PathBuf {
    let (root, main_file) = if let Some(file_path) = override_file {
        let path = PathBuf::from(file_path);
        let root = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        (root, path)
    } else {
        let root = match find_package_root() {
            Some(r) => r,
            None => {
                print_global_error(
                    "Could not find `pace.toml` in current directory or any parent directory",
                );
                exit(1);
            }
        };
        let main_file = root.join("src").join("main.pace");
        (root, main_file)
    };

    if !main_file.exists() {
        print_global_error(&format!("`{}` not found", main_file.display()));
        exit(1);
    }

    let ast_program = compile_to_mir(&main_file);

    if !ast_program.functions.contains_key("main") {
        print_global_error(
            "Entry point `main` not found. Executables require a `main` function or top-level statements.",
        );
        exit(1);
    }

    let generator = CraneliftGenerator::new();

    let profile_dir = if release { "release" } else { "debug" };
    let target_dir = root.join("target").join(profile_dir);
    if let Err(e) = fs::create_dir_all(&target_dir) {
        print_global_error(&format!("Failed to create target directory: {}", e));
        exit(1);
    }

    let obj_file = target_dir.join("out.o");
    if let Err(e) = generator.compile_program(&ast_program, &obj_file, release) {
        print_global_error(&format!("Codegen failed: {}", e));
        exit(1);
    }

    let package_name = if let Some(file_path) = override_file {
        Path::new(file_path).file_stem().and_then(|n| n.to_str()).unwrap_or("app")
    } else {
        root.file_name().and_then(|n| n.to_str()).unwrap_or("app")
    };
    let out_file = target_dir.join(package_name);
    if let Err(e) = Linker::link(&obj_file, &out_file, release) {
        print_global_error(&format!("Linker failed: {}", e));
        exit(1);
    }

    out_file
}
