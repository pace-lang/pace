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
use diagnostics::{Severity, print_diagnostics, DiagnosticBuilder, DiagnosticCode, SourceMap};

fn print_global_error(message: &str) {
    let diag = DiagnosticBuilder::global_error(DiagnosticCode::Custom("E001".into()), message).build();
    print_diagnostics(&[diag], &SourceMap::new());
}

#[derive(Parser)]
#[command(name = "pace")]
#[command(version)]
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
    /// Compile and run the current package or a specific file
    Run {
        /// Optional file to run
        file: Option<String>,
        #[arg(long)]
        /// Build artifacts in release mode, with optimizations
        release: bool,
    },
    /// Compile the current package or a specific file
    Build {
        /// Optional file to build
        file: Option<String>,
        #[arg(long)]
        /// Build artifacts in release mode, with optimizations
        release: bool,
    },
    /// Analyze the current package and report errors, but don't build object files
    Check,
    #[command(hide = true)]
    /// Run the tests
    Test,
    #[command(hide = true)]
    /// Format all pace files
    Fmt,
    #[command(hide = true)]
    /// Lint the current package
    Lint,
    
    #[command(hide = true)]
    /// Add dependencies to pace.toml
    Add {
        /// Name of the dependency to add
        dep: String,
    },
    #[command(hide = true)]
    /// Remove dependencies from pace.toml
    Remove {
        /// Name of the dependency to remove
        dep: String,
    },
    #[command(hide = true)]
    /// Download all dependencies locally
    Install,
    #[command(hide = true)]
    /// Update project package dependencies
    Update,
    /// Upgrade the Pace language SDK itself
    Upgrade,
    #[command(hide = true)]
    /// Display the dependency tree
    Tree,
    
    #[command(hide = true)]
    /// Package the project into a distributable format
    Package,
    #[command(hide = true)]
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
        print_global_error("File must have a .pace extension");
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
                print_global_error(&format!("Package Error: {}", diag.message));
            }
            exit(1);
        }
        Some(package_manager.into_graph())
    } else {
        package_manager.load_dummy_root();
        if !package_manager.errors.is_empty() {
            for diag in &package_manager.errors {
                print_global_error(&format!("Package Error: {}", diag.message));
            }
            exit(1);
        }
        Some(package_manager.into_graph())
    };

    let mut loader = module::loader::ModuleLoader::new(package_graph.as_ref());
    loader.load_root(file);
    
    let loader_errors = std::mem::take(&mut loader.errors);
    let (graph, source_map) = loader.into_graph();
    
    let mut has_errors = false;

    if !loader_errors.is_empty() {
        print_diagnostics(&loader_errors, &source_map);
        if loader_errors.iter().any(|d| d.severity == Severity::Error) {
            has_errors = true;
        }
    }

    if has_errors { exit(1); }

    // 3. Name Resolution
    let mut resolver = Resolver::new();
    resolver.resolve_graph(&graph);
    if !resolver.errors.is_empty() {
        print_diagnostics(&resolver.errors, &source_map);
        if resolver.errors.iter().any(|d| d.severity == Severity::Error) {
            has_errors = true;
        }
    }

    if has_errors { exit(1); }

    // 4. Type Checking
    let mut typechecker = TypeChecker::new();
    let typed_ast = typechecker.check_graph(&graph);
    if !typechecker.errors.is_empty() {
        print_diagnostics(&typechecker.errors, &source_map);
        if typechecker.errors.iter().any(|d| d.severity == Severity::Error) {
            has_errors = true;
        }
    }

    if has_errors { exit(1); }

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
            print_global_error(&format!("Directory {:?} already contains a pace.toml", path));
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
    println!("     Created binary (application) `{}` package", name);
    println!("\nTo get started:");
    println!("    cd {}", name);
    println!("    pace run");
}

fn do_check() -> Option<PathBuf> {
    let root = match find_package_root() {
        Some(r) => r,
        None => {
            print_global_error("Could not find `pace.toml` in current directory or any parent directory");
            exit(1);
        }
    };
    
    let main_file = root.join("src").join("main.pace");
    if !main_file.exists() {
        print_global_error("`src/main.pace` not found in package");
        exit(1);
    }
    
    // We run compile_to_mir just to parse/resolve/typecheck
    // Later we can separate compile_to_mir into check and lower.
    let _ = compile_to_mir(&main_file);
    println!("Check completed successfully.");
    Some(main_file)
}

fn do_build(override_file: Option<&str>, release: bool) -> PathBuf {
    let (root, main_file) = if let Some(file_path) = override_file {
        let path = PathBuf::from(file_path);
        let root = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        (root, path)
    } else {
        let root = match find_package_root() {
            Some(r) => r,
            None => {
                print_global_error("Could not find `pace.toml` in current directory or any parent directory");
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
        print_global_error("Entry point `main` not found. Executables require a `main` function or top-level statements.");
        exit(1);
    }

    let generator = CraneliftGenerator::new();
    
    // Ensure target output directory exists
    let profile_dir = if release { "release" } else { "debug" };
    let target_dir = root.join("target").join(profile_dir);
    fs::create_dir_all(&target_dir).unwrap();
    
    let obj_file = target_dir.join("out.o");
    if let Err(e) = generator.compile_program(&ast_program, &obj_file, release) {
        print_global_error(&format!("Codegen failed: {}", e));
        exit(1);
    }

    let package_name = root.file_name().and_then(|n| n.to_str()).unwrap_or("app");
    let out_file = target_dir.join(package_name);
    if let Err(e) = Linker::link(&obj_file, &out_file, release) {
        print_global_error(&format!("Linker failed: {}", e));
        exit(1);
    }
    
    if release {
        println!("    Finished `release` profile [optimized] target(s)");
    } else {
        println!("    Finished `dev` profile [unoptimized + debuginfo] target(s)");
    }
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
            let name = match current_dir.file_name().and_then(|n| n.to_str()) {
                Some(n) => n,
                None => {
                    eprintln!("[E002] Error: Cannot determine package name from current directory. Please use `pace new <name>` instead.");
                    std::process::exit(1);
                }
            };
            create_project(&current_dir, name);
        }
        Commands::Check => {
            do_check();
        }
        Commands::Build { file, release } => {
            do_build(file.as_deref(), *release);
        }
        Commands::Run { file, release } => {
            let out_file = do_build(file.as_deref(), *release);
            let package_name = out_file.file_name().and_then(|n| n.to_str()).unwrap_or("app");
            let profile_dir = if *release { "release" } else { "debug" };
            println!("     Running `target/{}/{}`", profile_dir, package_name);
            let status = Command::new(out_file.to_str().unwrap())
                .status()
                .expect("Failed to execute process");
                
            exit(status.code().unwrap_or(1));
        }
        Commands::DebugRun { file } => {
            let ast_program = compile_to_mir(file);

            if !ast_program.functions.contains_key("main") {
                print_global_error("Entry point `main` not found. Executables require a `main` function or top-level statements.");
                exit(1);
            }

            let mut vm = VirtualMachine::new(&ast_program);
            let result = vm.execute();
            if let Some(val) = result
                && val != mir::Value::Void {
                    println!("Result: {:?}", val);
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
        Commands::Upgrade => {
            println!("Upgrading Pace language SDK...");
            
            #[cfg(unix)]
            let status = Command::new("sh")
                .arg("-c")
                .arg("curl -sSfL https://raw.githubusercontent.com/pace-lang/pace/main/installer/install.sh | sh")
                .status();

            #[cfg(windows)]
            let status = Command::new("powershell")
                .arg("-Command")
                .arg("irm https://raw.githubusercontent.com/pace-lang/pace/main/installer/install.ps1 | iex")
                .status();

            match status {
                Ok(s) if s.success() => println!("Upgrade completed successfully!"),
                _ => {
                    print_global_error("Failed to upgrade Pace. Please check your internet connection or try reinstalling manually.");
                    exit(1);
                }
            }
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
                print_global_error("Could not find `pace.toml` in current directory or any parent directory");
            }
        }
    }
}
