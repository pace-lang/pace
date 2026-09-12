use clap::{Parser as ClapParser, Subcommand};
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use pace_driver::compile_file;

#[derive(ClapParser)]
#[command(
    name = "pace",
    version = "0.1.0",
    about = "Pace Toolchain and Package Manager"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new Pace package
    New {
        /// Name of the package
        name: String,
        /// Use a library template
        #[arg(long)]
        lib: bool,
    },
    /// Build the current package
    Build {
        /// Optional path to a specific .pace file to build in script mode
        file: Option<String>,
    },
    /// Typecheck the current package
    Check {
        /// Optional path to a specific .pace file to check in script mode
        file: Option<String>,
    },
    /// Build and run the current package
    Run {
        /// Optional path to a specific .pace file to run in script mode
        file: Option<String>,
    },
    /// Print the toolchain version
    Version,
    /// Upgrade the toolchain to the latest version
    Upgrade,
}

fn get_project_info() -> Result<(PathBuf, String), String> {
    let current_dir = env::current_dir().map_err(|_| "Failed to get current directory")?;
    let manifest_path = pace_pkg::find_manifest(&current_dir)
        .ok_or("No pace.toml found in this directory or any parent directory")?;

    let toml = pace_pkg::parse_manifest(&manifest_path)?;
    let root = manifest_path.parent().unwrap().to_path_buf();

    Ok((root, toml.package.name))
}

// compile_file is now in pace_driver

fn execute_build_or_run(file: Option<String>, run: bool, check: bool) -> Result<(), String> {
    if let Some(f) = file {
        let p = PathBuf::from(&f);
        let name = p.file_stem().unwrap().to_str().unwrap().to_string();
        compile_file(&p, Path::new("."), &name, run, check)
    } else {
        let (root, project_name) = get_project_info()?;

        let manifest_path = root.join("pace.toml");
        let toml = pace_pkg::parse_manifest(&manifest_path)?;
        
        let mut resolver = pace_pkg::resolve::DependencyResolver::new();
        let lock = resolver.resolve(&toml)?;
        resolver.write_lockfile(&root.join("pace.lock"), &lock)?;

        let main_file = root.join("src/main.pace");
        let lib_file = root.join("src/lib.pace");

        let target_file = if main_file.exists() {
            main_file
        } else if lib_file.exists() {
            lib_file
        } else {
            return Err("Neither src/main.pace nor src/lib.pace found".to_string());
        };

        let build_dir = root.join("build");
        compile_file(&target_file, &build_dir, &project_name, run, check)
    }
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::New { name, lib } => {
            if let Err(e) = pace_pkg::scaffold_project(&name, lib) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            println!("Created new package '{}'", name);
        }
        Commands::Build { file } => {
            if let Err(e) = execute_build_or_run(file, false, false) {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        Commands::Check { file } => {
            if let Err(e) = execute_build_or_run(file, false, true) {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        Commands::Run { file } => {
            if let Err(e) = execute_build_or_run(file, true, false) {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        Commands::Version => {
            println!("pace version 0.1.0");
        }
        Commands::Upgrade => {
            println!("Upgrading Pace toolchain from GitHub...");
            let status = Command::new("sh")
                .arg("-c")
                .arg("curl -sSf https://raw.githubusercontent.com/pace-lang/pace/main/installer/install.sh | sh")
                .status()
                .expect("Failed to run upgrade script");
            if status.success() {
                println!("Upgrade complete!");
            } else {
                eprintln!("Upgrade failed.");
                std::process::exit(1);
            }
        }
    }
}
