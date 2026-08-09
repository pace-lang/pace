use clap::{Parser, Subcommand};
use std::fs;
use std::path::PathBuf;
use std::process::exit;

use lexer::{Scanner, TokenKind};
use parser::Parser as PaceParser;
use resolver::Resolver;
use typechecker::TypeChecker;
use lowering::ProgramBuilder;
use vm::VirtualMachine;

#[derive(Parser)]
#[command(name = "pace")]
#[command(about = "The Pace Compiler", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a .pace file directly via the Virtual Machine
    Run {
        /// The .pace file to run
        #[arg(value_name = "FILE")]
        file: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Run { file } => {
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
            let mir_program = builder.build(&ast);

            // 6. Execution (VM)
            let mut vm = VirtualMachine::new(&mir_program);
            let result = vm.execute();

            if let Some(val) = result {
                if val != mir::Value::Void {
                    println!("Result: {:?}", val);
                }
            }
        }
    }
}
