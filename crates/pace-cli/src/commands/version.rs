use miette::Result;
use colored::Colorize;

pub fn execute() -> Result<()> {
    println!("{} {}", "pace".green().bold(), env!("CARGO_PKG_VERSION"));
    Ok(())
}
