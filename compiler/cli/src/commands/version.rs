pub fn execute() {
    use colored::Colorize;
    let version = env!("CARGO_PKG_VERSION");
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    
    println!("{} v{} ({}-{})", "pace".green().bold(), version.cyan(), os, arch);
}
