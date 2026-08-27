pub mod commands;
pub mod utils;

use clap::{Parser, Subcommand};
use miette::Result;
use pace_driver::CompilerSession;

#[derive(Parser)]
#[command(name = "pace")]
#[command(about = "The Pace Programming Language", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new Pace project in a new directory
    New {
        /// The name of the project
        name: String,
        /// Create a package instead of an executable project
        #[arg(long)]
        pkg: bool,
    },
    /// Initialize a new Pace project in the current directory
    Init {
        /// Create a package instead of an executable project
        #[arg(long)]
        pkg: bool,
    },
    /// Fetch and update dependencies from pace.toml
    Fetch,
    /// Add a new dependency to pace.toml
    Add {
        /// Name of the package to add
        name: String,
        /// Path to a local package
        #[arg(long)]
        path: Option<String>,
        /// Version string (e.g. 1.0.0)
        #[arg(long)]
        version: Option<String>,
    },
    /// Remove a dependency from pace.toml
    Remove {
        /// Name of the package to remove
        name: String,
    },
    /// Package and upload the current project to the Pace Registry
    Publish {
        /// Perform a dry run without actually uploading
        #[arg(long)]
        dry_run: bool,
    },
    /// Authenticate with the Pace Registry
    Login {
        /// The token to authenticate with
        token: String,
    },
    /// Check a Pace file for syntax and type errors
    Check {
        /// The Pace file to check (optional, defaults to src/main.pace if pace.toml exists)
        file: Option<String>,
        /// Output format (e.g. "json" or "human")
        #[arg(long, default_value = "human")]
        output_format: String,
    },
    /// Compile a Pace file into an executable
    Build {
        /// The Pace file to build (optional, defaults to src/main.pace if pace.toml exists)
        file: Option<String>,
        /// Build with optimizations enabled
        #[arg(long)]
        release: bool,
    },
    /// Compile and run a Pace file
    Run {
        /// The Pace file to run (optional, defaults to src/main.pace if pace.toml exists)
        file: Option<String>,
        /// Run with optimizations enabled
        #[arg(long)]
        release: bool,
    },
    /// Start the Pace Language Server
    Lsp,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Commands::Check { output_format, .. } = &cli.command {
        if output_format == "json" {
            miette::set_hook(Box::new(|_| {
                Box::new(miette::JSONReportHandler::new())
            })).unwrap();
        }
    }

    let session = CompilerSession::new();

    match cli.command {
        Commands::New { name, pkg } => commands::new::execute(name, pkg)?,
        Commands::Init { pkg } => commands::init::execute(pkg)?,
        Commands::Fetch => commands::fetch::execute()?,
        Commands::Add { name, path, version } => commands::add::execute(name, path, version)?,
        Commands::Remove { name } => commands::remove::execute(name)?,
        Commands::Publish { dry_run } => commands::publish::execute(&session, dry_run)?,
        Commands::Login { token } => commands::login::login(token)?,
        Commands::Check { file, output_format } => commands::check::execute(&session, file, output_format)?,
        Commands::Build { file, release } => commands::build::execute(&session, file, release)?,
        Commands::Run { file, release } => commands::run::execute(&session, file, release)?,
        Commands::Lsp => pace_lsp::run_server(),
    }

    Ok(())
}
