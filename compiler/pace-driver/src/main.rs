use std::env;
use std::fs;
use std::process::Command;

use pace_lexer::Lexer;
use pace_parser::Parser;
use pace_hir::LoweringContext;
use pace_ty::TypeChecker;
use pace_mir::MirBuilder;
use pace_codegen::CGenerator;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: pace build <file.pace>");
        std::process::exit(1);
    }

    let command = &args[1];
    let file_path = &args[2];

    if command != "build" {
        eprintln!("Unknown command: {}", command);
        std::process::exit(1);
    }

    // 1. Read source
    let source = fs::read_to_string(file_path).expect("Failed to read file");

    // 2. Lex & Parse
    let lexer = Lexer::new(&source);
    let mut parser = Parser::new(lexer);
    let ast = parser.parse_program().expect("Failed to parse program");

    // 3. Lower to HIR
    let mut lowerer = LoweringContext::new();
    let hir = lowerer.lower_program(ast).expect("Failed to lower to HIR");

    // 4. Typecheck
    let mut tc = TypeChecker::new();
    tc.check_program(&hir).expect("Typecheck failed");

    // 5. Build MIR
    let builder = MirBuilder::new();
    let mir = builder.build_program(&hir);

    // 6. Generate C Code
    let mut codegen = CGenerator::new();
    let c_code = codegen.generate(&mir);

    let c_file = "/tmp/pace_out.c";
    let bin_file = "hello"; // Output binary name
    
    fs::write(c_file, c_code).expect("Failed to write C file");

    // 7. Compile with GCC
    println!("Compiling {}...", file_path);
    let status = Command::new("gcc")
        .arg(c_file)
        .arg("../runtime/pace_runtime.c")
        .arg("-I../runtime")
        .arg("-o")
        .arg(bin_file)
        .status()
        .expect("Failed to invoke gcc");

    if status.success() {
        println!("Successfully compiled to ./{}", bin_file);
    } else {
        eprintln!("C compilation failed!");
        std::process::exit(1);
    }
}
