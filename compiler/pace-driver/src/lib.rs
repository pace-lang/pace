use std::fs;
use std::path::Path;
use std::process::Command;

use pace_lexer::Lexer;
use pace_parser::Parser;
use pace_hir::LoweringContext;
use pace_ty::TypeChecker;
use pace_mir::MirBuilder;
use pace_codegen::CGenerator;

pub fn compile_file(file_path: &Path, output_dir: &Path, output_name: &str, run: bool, check_only: bool) -> Result<(), String> {
    if !file_path.exists() {
        return Err(format!("File not found: {}", file_path.display()));
    }

    let source = fs::read_to_string(file_path).map_err(|e| format!("Failed to read file: {}", e))?;

    // 1. Lex & Parse
    let lexer = Lexer::new(&source);
    let mut parser = Parser::new(lexer);
    let ast = match parser.parse_program() {
        Ok(ast) => ast,
        Err(diag) => {
            let mut reporter = pace_errors::Reporter::new();
            reporter.report(diag);
            reporter.emit_all(&source, file_path.to_str().unwrap());
            return Err("Compilation failed due to syntax errors.".to_string());
        }
    };

    // 2. Lower to HIR
    let mut lowerer = LoweringContext::new();
    let hir = lowerer.lower_program(ast)?;

    // 3. Typecheck
    let mut tc = TypeChecker::new();
    if let Err(e) = tc.check_program(&hir) {
        let mut reporter = pace_errors::Reporter::new();
        reporter.report(pace_errors::Diagnostic::error(e));
        reporter.emit_all(&source, file_path.to_str().unwrap());
        tc.reporter.emit_all(&source, file_path.to_str().unwrap());
        return Err("Compilation failed due to type errors.".to_string());
    }
    
    tc.reporter.emit_all(&source, file_path.to_str().unwrap());
    if tc.reporter.has_errors() {
        return Err("Compilation failed due to type errors.".to_string());
    }

    if check_only {
        println!("Check finished successfully.");
        return Ok(());
    }

    // 4. Build MIR
    let mir = MirBuilder::build_program(&hir, &mut tc);

    // 5. Generate C Code
    let mut codegen = CGenerator::new();
    let c_code = codegen.generate(&mir);

    let c_file = "/tmp/pace_out.c";
    fs::write(c_file, c_code).map_err(|e| format!("Failed to write C file: {}", e))?;

    // 6. Compile with GCC
    fs::create_dir_all(output_dir).map_err(|e| format!("Failed to create output directory: {}", e))?;
    let bin_file = output_dir.join(output_name);
    
    // Find runtime path
    let runtime_path = if Path::new("runtime/pace_runtime.c").exists() {
        "runtime/pace_runtime.c".to_string()
    } else if Path::new("../runtime/pace_runtime.c").exists() {
        "../runtime/pace_runtime.c".to_string()
    } else if Path::new("../../runtime/pace_runtime.c").exists() {
        "../../runtime/pace_runtime.c".to_string()
    } else {
        "runtime/pace_runtime.c".to_string()
    };

    let status = Command::new("gcc")
        .arg("-O2")
        .arg(format!("-I{}", Path::new(&runtime_path).parent().unwrap().display()))
        .arg(c_file)
        .arg(&runtime_path)
        .arg("-o")
        .arg(&bin_file)
        .status()
        .map_err(|e| format!("Failed to invoke gcc: {}", e))?;

    if !status.success() {
        return Err("GCC compilation failed".to_string());
    }

    if !run {
        let file_name = file_path.file_name().unwrap_or_default().to_string_lossy();
        let bin_name = bin_file.file_name().unwrap_or_default().to_string_lossy();
        println!("build {} into ./build/{}", file_name, bin_name);
    }

    if run {
        let run_status = Command::new(&bin_file)
            .status()
            .map_err(|e| format!("Failed to run executable: {}", e))?;
        if !run_status.success() {
            return Err("Execution failed".to_string());
        }
    }

    Ok(())
}
