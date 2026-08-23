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
            let output_name = std::path::Path::new(&file)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            
            let output = if output_name.is_empty() { "output".to_string() } else { output_name };
            
            println!("Building {} to ./{}...", file, output);
            session.build_file(&file, &output)?;
            println!("✅ Build complete!");
        }
        Commands::Run { file } => {
            session.run_file(&file)?;
        }
    }

    Ok(())
}
