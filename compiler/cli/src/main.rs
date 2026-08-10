use clap::{Parser, Subcommand};
use std::fs;
use std::path::{PathBuf, Path};
use std::process::{exit, Command};


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
    
    // Find pace.toml root
    let mut current_dir = file.canonicalize().unwrap_or(file.to_path_buf());
    let mut package_root = None;
    while let Some(parent) = current_dir.parent() {
        if parent.join("pace.toml").exists() {
            package_root = Some(parent.to_path_buf());
            break;
        }
        current_dir = parent.to_path_buf();
    }
    
    let mut package_manager = package::manager::PackageManager::new();
    let package_graph = if let Some(root) = package_root {
        package_manager.load_root(&root);
        if !package_manager.errors.is_empty() {
            for diag in &package_manager.errors {
                eprintln!("Package Error: {}", diag.message);
            }
            exit(1);
        }
        Some(package_manager.into_graph())
    } else {
        None
    };

    let mut loader = module::loader::ModuleLoader::new(package_graph.as_ref());
    loader.load_root(file);
    if !loader.errors.is_empty() {
        for diag in &loader.errors {
            eprintln!("Error [{}]: {} at line {}", diag.code.as_str(), diag.message, diag.primary_span.start_loc.line);
        }
        exit(1);
    }

    let graph = loader.into_graph();

    // 3. Name Resolution
    let mut resolver = Resolver::new();
    resolver.resolve_graph(&graph);
    if !resolver.errors.is_empty() {
        for diag in &resolver.errors {
            eprintln!("Error [{}]: {} at line {}", diag.code.as_str(), diag.message, diag.primary_span.start_loc.line);
        }
        exit(1);
    }

    // 4. Type Checking
    let mut typechecker = TypeChecker::new();
    let typed_ast = typechecker.check_graph(&graph);
    if !typechecker.errors.is_empty() {
        for diag in &typechecker.errors {
            eprintln!("Error [{}]: {} at line {}", diag.code.as_str(), diag.message, diag.primary_span.start_loc.line);
        }
        exit(1);
    }

    // 5. Lowering (AST -> MIR)
    let builder = ProgramBuilder::new();
    let mut mir_program = builder.build(&typed_ast);
    
    // 6. ARC Pass
    let arc_pass = arc::arc_pass::ArcPass::new();
    arc_pass.run(&mut mir_program);
    
    // println!("{:#?}", mir_program);
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
            if file.file_name().and_then(|s| s.to_str()) == Some("pace.toml") {
                let manifest_content = fs::read_to_string(file).unwrap_or_else(|e| {
                    eprintln!("Failed to read pace.toml: {}", e);
                    exit(1);
                });
                let manifest: package::manifest::Manifest = toml::from_str(&manifest_content).unwrap_or_else(|e| {
                    eprintln!("Failed to parse pace.toml: {}", e);
                    exit(1);
                });
                println!("Loaded package manifest: {} v{}", manifest.package.name, manifest.package.version);
                println!("Multi-file compilation is being implemented. Please build individual .pace files for now.");
                return;
            }

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
