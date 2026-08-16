use std::path::PathBuf;

pub fn find_package_root() -> Option<PathBuf> {
    let mut current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    loop {
        if current_dir.join("pace.toml").exists() {
            return Some(current_dir);
        }
        if !current_dir.pop() {
            break;
        }
    }
    None
}
