use colored::Colorize;
use miette::Result;
use std::process::Command;

pub fn execute() -> Result<()> {
    println!("{}", "🔄 Upgrading Pace SDK...".cyan());

    let status = Command::new("sh")
        .arg("-c")
        .arg("curl -fsSL https://raw.githubusercontent.com/pace-lang/pace/main/installer/install.sh | sh")
        .status()
        .map_err(|e| miette::miette!("Failed to execute installer: {}", e))?;

    if !status.success() {
        return Err(miette::miette!("Upgrade failed."));
    }

    Ok(())
}
