use colored::Colorize;
use miette::Result;

pub fn execute() -> Result<()> {
    println!("{} {}", "pace".green().bold(), env!("CARGO_PKG_VERSION"));
    Ok(())
}
