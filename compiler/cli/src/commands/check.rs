use std::path::PathBuf;
use std::process::exit;
use diagnostics::{Severity, print_diagnostics};

use crate::utils::errors::print_global_error;
use crate::utils::workspace::find_package_root;
use crate::utils::compiler::compile_to_mir;

pub fn execute() -> Option<PathBuf> {
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
    if !main_file.exists() {
        print_global_error("`src/main.pace` not found in package");
        exit(1);
    }

    let _ = compile_to_mir(&main_file);
    println!("Check completed successfully.");
    Some(main_file)
}

pub fn execute_lint() {
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
    if !main_file.exists() {
        print_global_error("`src/main.pace` not found in package");
        exit(1);
    }

    let mut package_manager = package::manager::PackageManager::new();
    package_manager.load_root(&root);
    if !package_manager.errors.is_empty() {
        for diag in &package_manager.errors {
            print_global_error(&format!("Package Error: {}", diag.message));
        }
        exit(1);
    }
    let package_graph = package_manager.into_graph();

    let session = session::CompilerSession::new();
    let mut loader = module::loader::ModuleLoader::new(Some(&package_graph), &session);
    loader.load_root(&main_file);

    let loader_errors = std::mem::take(&mut loader.errors);
    let (graph, source_map) = loader.into_graph();

    if !loader_errors.is_empty() {
        print_diagnostics(&loader_errors, &source_map);
        if loader_errors.iter().any(|d| d.severity == Severity::Error) {
            exit(1);
        }
    }

    let mut linter = linter::Linter::new(&session);
    for module in graph.modules() {
        linter.lint(&module.ast);
    }
    let linter_warnings = linter.into_diagnostics();

    if !linter_warnings.is_empty() {
        print_diagnostics(&linter_warnings, &source_map);
        exit(1);
    } else {
        println!("No style violations found.");
    }
}
