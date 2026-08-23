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
    /// Create a new Pace project
    New {
        /// The name of the project
        name: String,
    },
    /// Check a Pace file for syntax and type errors
    Check {
        /// The Pace file to check (optional, defaults to src/main.pace if pace.toml exists)
        file: Option<String>,
    },
    /// Compile a Pace file into an executable
    Build {
        /// The Pace file to build (optional, defaults to src/main.pace if pace.toml exists)
        file: Option<String>,
    },
    /// Compile and run a Pace file
    Run {
        /// The Pace file to run (optional, defaults to src/main.pace if pace.toml exists)
        file: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let session = CompilerSession::new();

    match cli.command {
        Commands::New { name } => commands::new::execute(name)?,
        Commands::Check { file } => commands::check::execute(&session, file)?,
        Commands::Build { file } => commands::build::execute(&session, file)?,
        Commands::Run { file } => commands::run::execute(&session, file)?,
    }

    Ok(())
}
