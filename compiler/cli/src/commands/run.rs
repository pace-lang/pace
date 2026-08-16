use std::path::PathBuf;
use std::process::{Command, exit};
use vm::VirtualMachine;

use crate::commands::build;
use crate::utils::compiler::compile_to_mir;
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

pub fn execute_debug(file: &PathBuf) {
    let ast_program = compile_to_mir(file);

    if !ast_program.functions.contains_key("main") {
        print_global_error(
            "Entry point `main` not found. Executables require a `main` function or top-level statements.",
        );
        exit(1);
    }

    let mut vm = VirtualMachine::new(&ast_program);
    let result = vm.execute();
    if let Some(val) = result
        && val != mir::Value::Void {
            println!("Result: {:?}", val);
        }
}
