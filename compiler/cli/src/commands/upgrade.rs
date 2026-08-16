use std::process::{Command, exit};

pub fn execute() {
    let status = Command::new("sh")
        .arg("-c")
        .arg("curl -fsSL https://raw.githubusercontent.com/pace-lang/pace/main/installer/install.sh | bash")
        .status();

    match status {
        Ok(status) if status.success() => {
            exit(0);
        }
        _ => {
            eprintln!("Failed to upgrade Pace.");
            exit(1);
        }
    }
}
