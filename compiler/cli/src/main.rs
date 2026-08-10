use clap::{Parser, Subcommand};
use std::fs;
use std::path::{PathBuf, Path};
use std::process::{exit, Command};

use lexer::{Scanner, TokenKind};
use parser::Parser as PaceParser;
use resolver::Resolver;
use typechecker::TypeChecker;
use lowering::ProgramBuilder;
use vm::VirtualMachine;
use codegen::CraneliftGenerator;
use linker::Linker;

#[derive(Parser)]
#[command(name = "pace")]
#[command(about = "The Pace Compiler", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compile and link a .pace file into a native executable
    Build {
        /// The .pace file to build
        #[arg(value_name = "FILE")]
        file: PathBuf,
    },
    /// Compile, link, and run a .pace file
    Run {
        /// The .pace file to run
        #[arg(value_name = "FILE")]
        file: PathBuf,
    },
    /// Run a .pace file directly via the Virtual Machine (development only)
    DebugRun {
        /// The .pace file to run
        #[arg(value_name = "FILE")]
        file: PathBuf,
    },
}

fn compile_to_mir(file: &Path) -> mir::Program {
    if file.extension().and_then(|e| e.to_str()) != Some("pace") {
        eprintln!("Error: File must have a .pace extension");
        exit(1);
    }

    let source = match fs::read_to_string(file) {
        Ok(content) => content,
        Err(err) => {
            eprintln!("Error reading file: {}", err);
            exit(1);
        }
    };

    // 1. Lexical Analysis
    let mut scanner = Scanner::new(&source);
    let tokens = scanner.scan_tokens();
    
    let mut has_lexer_errors = false;
    for token in &tokens {
        if let TokenKind::Error(msg) = &token.kind {
            eprintln!("Lexer error at line {}: {}", token.span.start_loc.line, msg);
            has_lexer_errors = true;
        }
    }
    if has_lexer_errors {
        exit(1);
    }

    // 2. Parsing
    let mut parser = PaceParser::new(tokens);
    let (ast, parse_errors) = parser.parse();
    if !parse_errors.is_empty() {
        eprintln!("Parse errors occurred:");
        for err in parse_errors {
            eprintln!("{}", err);
        }
        exit(1);
    }

    // 3. Name Resolution
    let mut resolver = Resolver::new();
    resolver.resolve(&ast);
    if !resolver.errors.is_empty() {
        eprintln!("Resolution errors occurred:");
        for err in &resolver.errors {
            eprintln!("{:?}", err);
        }
        exit(1);
    }

    // 4. Type Checking
    let mut typechecker = TypeChecker::new();
    typechecker.check(&ast);
    if !typechecker.errors.is_empty() {
        eprintln!("Type errors occurred:");
        for err in &typechecker.errors {
            eprintln!("{:?}", err);
        }
        exit(1);
    }

    // 5. Lowering (AST -> MIR)
    let builder = ProgramBuilder::new();
    let mut mir_program = builder.build(&ast);
    
    // 6. ARC Pass
    let arc_pass = arc::arc_pass::ArcPass::new();
    arc_pass.run(&mut mir_program);
    
    mir_program
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::DebugRun { file } => {
            let mir_program = compile_to_mir(file);
            // Execution (VM)
            let mut vm = VirtualMachine::new(&mir_program);
            let result = vm.execute();

            if let Some(val) = result {
                if val != mir::Value::Void {
                    println!("Result: {:?}", val);
                }
            }
        }
        Commands::Build { file } => {
            let mir_program = compile_to_mir(file);
            
            let obj_file = file.with_extension("o");
            let exe_file = file.with_extension(""); // e.g. test.pace -> test

            let generator = CraneliftGenerator::new();
            if let Err(e) = generator.compile_program(&mir_program, &obj_file) {
                eprintln!("Codegen error: {}", e);
                exit(1);
            }

            if let Err(e) = Linker::link(&obj_file, &exe_file) {
                eprintln!("Linker error: {}", e);
                exit(1);
            }

            println!("Successfully built {:?}", exe_file);
        }
        Commands::Run { file } => {
            let mir_program = compile_to_mir(file);
            
            let obj_file = file.with_extension("o");
            let exe_file = file.with_extension("");

            let generator = CraneliftGenerator::new();
            if let Err(e) = generator.compile_program(&mir_program, &obj_file) {
                eprintln!("Codegen error: {}", e);
                exit(1);
            }

            if let Err(e) = Linker::link(&obj_file, &exe_file) {
                eprintln!("Linker error: {}", e);
                exit(1);
            }

            // Run the executable
            let mut exe_path = PathBuf::from(".");
            exe_path.push(&exe_file);
            let status = Command::new(&exe_path)
                .status()
                .unwrap_or_else(|e| {
                    eprintln!("Failed to execute {:?}: {}", exe_file, e);
                    exit(1);
                });
                
            exit(status.code().unwrap_or(1));
        }
    }
}
