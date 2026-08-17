use clap::{Parser, Subcommand};
use std::path::PathBuf;

pub mod commands;
pub mod utils;

#[derive(Parser)]
#[command(name = "pace")]
#[command(version = get_version())]
#[command(about = "The Pace Compiler", long_about = None)]
#[command(styles = get_styles())]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

fn get_styles() -> clap::builder::styling::Styles {
    use clap::builder::styling::{AnsiColor, Effects, Styles};
    Styles::styled()
        .header(AnsiColor::Green.on_default() | Effects::BOLD)
        .usage(AnsiColor::Green.on_default() | Effects::BOLD)
        .literal(AnsiColor::Cyan.on_default() | Effects::BOLD)
        .placeholder(AnsiColor::Cyan.on_default())
        .error(AnsiColor::Red.on_default() | Effects::BOLD)
        .valid(AnsiColor::Green.on_default() | Effects::BOLD)
        .invalid(AnsiColor::Red.on_default() | Effects::BOLD)
}

fn get_version() -> &'static str {
    use colored::Colorize;
    let version = env!("CARGO_PKG_VERSION");
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let v_str = format!("v{} ({}-{})", version.cyan(), os, arch);
    Box::leak(v_str.into_boxed_str())
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new pace project
    Create {
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
    /// Display version information
    Version,
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


}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Create { name } => {
            let path = PathBuf::from(name);
            commands::new::execute(&path, name);
        }
        Commands::Init => {
            let current_dir = match std::env::current_dir() {
                Ok(dir) => dir,
                Err(e) => {
                    eprintln!("[E002] Error: Failed to determine current directory: {}", e);
                    std::process::exit(1);
                }
            };
            let name = match current_dir.file_name().and_then(|n| n.to_str()) {
                Some(n) => n,
                None => {
                    eprintln!(
                        "[E002] Error: Cannot determine package name from current directory. Please use `pace new <name>` instead."
                    );
                    std::process::exit(1);
                }
            };
            commands::new::execute(&current_dir, name);
        }
        Commands::Check => {
            commands::check::execute();
        }
        Commands::Lint => {
            commands::check::execute_lint();
        }
        Commands::Build { file, release } => {
            use colored::Colorize;
            let out_file = commands::build::execute(file.as_deref(), *release);
            println!("{}", out_file.display().to_string().green().bold());
        }
        Commands::Run { file, release } => {
            commands::run::execute(file.as_deref(), *release);
        }
        Commands::Test => {
            println!("Not implemented yet");
        }
        Commands::Fmt => {
            println!("Not implemented yet");
        }
        Commands::Add { .. } => println!("Not implemented yet"),
        Commands::Remove { .. } => println!("Not implemented yet"),
        Commands::Install => println!("Not implemented yet"),
        Commands::Update => println!("Not implemented yet"),
        Commands::Upgrade => {
            commands::upgrade::execute();
        }
        Commands::Version => {
            commands::version::execute();
        }
        Commands::Tree => println!("Not implemented yet"),
        Commands::Package => println!("Not implemented yet"),
        Commands::Publish => println!("Not implemented yet"),
        Commands::Clean => println!("Not implemented yet"),
    }
}
