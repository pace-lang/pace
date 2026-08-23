use clap::{Parser, Subcommand};
use miette::{Result, IntoDiagnostic};
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
    /// Check a Pace file for syntax and type errors
    Check {
        /// The Pace file to check
        file: String,
    },
    /// Compile a Pace file into an executable
    Build {
        /// The Pace file to build
        file: String,
    },
    /// Compile and run a Pace file
    Run {
        /// The Pace file to run
        file: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let session = CompilerSession::new();

    match cli.command {
        Commands::Check { file } => {
            println!("Checking {}...", file);
            let ast = session.check_file(&file)?;
            println!("✅ Syntax OK");
            println!("{:#?}", ast);
        }
        Commands::Build { file } => {
            println!("Building {}... (Not yet implemented)", file);
        }
        Commands::Run { file } => {
            println!("Running {}... (Not yet implemented)", file);
        }
    }

    Ok(())
}
