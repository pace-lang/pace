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
    /// Create a new pace project
    New {
        /// Name of the project
        name: String,
    },
    /// Initialize a new pace project in the current directory
    Init,
    /// Compile and run the current package
    Run,
    /// Compile the current package
    Build,
    /// Analyze the current package and report errors, but don't build object files
    Check,
    /// Run the tests
    Test,
    /// Format all pace files
    Fmt,
    /// Lint the current package
    Lint,
    
    /// Add dependencies to pace.toml
    Add {
        /// Name of the dependency to add
        dep: String,
    },
    /// Remove dependencies from pace.toml
    Remove {
        /// Name of the dependency to remove
        dep: String,
    },
    /// Download all dependencies locally
    Install,
    /// Update dependencies
    Update,
    /// Display the dependency tree
    Tree,
    
    /// Package the project into a distributable format
    Package,
    /// Publish the package to the registry
    Publish,
    /// Remove the target directory and built artifacts
    Clean,

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

fn find_package_root() -> Option<PathBuf> {
    let mut current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    loop {
        if current_dir.join("pace.toml").exists() {
            return Some(current_dir);
        }
        if !current_dir.pop() {
            break;
        }
    }
    None
}

fn create_project(path: &Path, name: &str) {
    if path.exists() {
        if path.read_dir().map(|mut i| i.next().is_some()).unwrap_or(false) && path.join("pace.toml").exists() {
            eprintln!("Error: Directory {:?} already contains a pace.toml", path);
            exit(1);
        }
    } else {
        fs::create_dir_all(path).unwrap();
    }
    
    fs::create_dir_all(path.join("src")).unwrap();
    let toml = format!(
r#"[package]
name = "{}"
version = "0.1.0"

[dependencies]
"#, name);
    fs::write(path.join("pace.toml"), toml).unwrap();
    
    let main_pace = 
r#"func main() {
    print("✨ Welcome to Pace — let's make something great.");
}
"#;
    fs::write(path.join("src").join("main.pace"), main_pace).unwrap();
    println!("Created new package `{}`", name);
}

fn do_check() -> Option<PathBuf> {
    let root = match find_package_root() {
        Some(r) => r,
        None => {
            eprintln!("Error: could not find `pace.toml` in current directory or any parent directory");
            exit(1);
        }
    };
    
    let main_file = root.join("src").join("main.pace");
    if !main_file.exists() {
        eprintln!("Error: `src/main.pace` not found in package");
        exit(1);
    }
    
    // We run compile_to_mir just to parse/resolve/typecheck
    // Later we can separate compile_to_mir into check and lower.
    let _ = compile_to_mir(&main_file);
    println!("Check completed successfully.");
    Some(main_file)
}

fn do_build() -> PathBuf {
    let root = match find_package_root() {
        Some(r) => r,
        None => {
            eprintln!("Error: could not find `pace.toml` in current directory or any parent directory");
            exit(1);
        }
    };
    
    let main_file = root.join("src").join("main.pace");
    if !main_file.exists() {
        eprintln!("Error: `src/main.pace` not found in package");
        exit(1);
    }
    
    let ast_program = compile_to_mir(&main_file);

    let generator = CraneliftGenerator::new();
    
    // Ensure target/debug exists
    let target_dir = root.join("target").join("debug");
    fs::create_dir_all(&target_dir).unwrap();
    
    let obj_file = target_dir.join("out.o");
    if let Err(e) = generator.compile_program(&ast_program, &obj_file) {
        eprintln!("Codegen error: {}", e);
        exit(1);
    }

    let package_name = root.file_name().and_then(|n| n.to_str()).unwrap_or("app");
    let out_file = target_dir.join(package_name);
    if let Err(e) = Linker::link(&obj_file, &out_file) {
        eprintln!("Linker error: {}", e);
        exit(1);
    }
    
    println!("    Finished `dev` profile [unoptimized + debuginfo] target(s)");
    out_file
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::New { name } => {
            let path = PathBuf::from(name);
            create_project(&path, name);
        }
        Commands::Init => {
            let current_dir = std::env::current_dir().unwrap();
            let name = current_dir.file_name().unwrap().to_str().unwrap();
            create_project(&current_dir, name);
        }
        Commands::Check => {
            do_check();
        }
        Commands::Build => {
            do_build();
        }
        Commands::Run => {
            let out_file = do_build();
            let root = find_package_root().unwrap();
            let package_name = root.file_name().and_then(|n| n.to_str()).unwrap_or("app");
            println!("     Running `target/debug/{}`", package_name);
            let status = Command::new(out_file.to_str().unwrap())
                .status()
                .expect("Failed to execute process");
                
            exit(status.code().unwrap_or(1));
        }
        Commands::DebugRun { file } => {
            let ast_program = compile_to_mir(file);

            let mut vm = VirtualMachine::new(&ast_program);
            let result = vm.execute();
            if let Some(val) = result {
                if val != mir::Value::Void {
                    println!("Result: {:?}", val);
                }
            }
        }
        Commands::Test => {
            println!("Not implemented yet");
        }
        Commands::Fmt => {
            println!("Not implemented yet");
        }
        Commands::Lint => {
            println!("Not implemented yet");
        }
        Commands::Add { .. } => {
            println!("Not implemented yet");
        }
        Commands::Remove { .. } => {
            println!("Not implemented yet");
        }
        Commands::Install => {
            println!("Not implemented yet");
        }
        Commands::Update => {
            println!("Not implemented yet");
        }
        Commands::Tree => {
            println!("Not implemented yet");
        }
        Commands::Publish => {
            println!("Not implemented yet");
        }
        Commands::Package => {
            println!("Not implemented yet");
        }
        Commands::Clean => {
            if let Some(root) = find_package_root() {
                let target = root.join("target");
                if target.exists() {
                    fs::remove_dir_all(&target).unwrap();
                    println!("Cleaned `target` directory.");
                } else {
                    println!("Nothing to clean.");
                }
            } else {
                eprintln!("Error: could not find `pace.toml` in current directory or any parent directory");
            }
        }
    }
}
