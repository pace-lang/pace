use miette::Result;
use pace_pkg::fetcher::Fetcher;
use colored::Colorize;

pub fn execute() -> Result<()> {
    let current_dir = std::env::current_dir().map_err(|e| miette::miette!("Failed to get current dir: {}", e))?;
    
    println!("{}", "⬇️  Fetching packages...".cyan());
    let fetcher = Fetcher::new()?;
    fetcher.fetch(&current_dir)?;
    println!("{}", "✅ Packages fetched successfully.".green().bold());
    
    Ok(())
}
