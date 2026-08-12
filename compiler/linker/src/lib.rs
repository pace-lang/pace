use std::path::Path;
use std::process::Command;
use std::env;

pub struct Linker;

impl Linker {
    pub fn link(object_file: &Path, output_file: &Path) -> Result<(), String> {
        // Find the Rust runtime staticlib (libpace_runtime.a)
        // It should be in the same directory as the compiler executable (e.g., target/debug)
        let exe_path = env::current_exe().map_err(|e| format!("Failed to get current executable path: {}", e))?;
        let exe_dir = exe_path.parent().ok_or("Executable has no parent directory")?;
        
        let mut search_paths = vec![
            exe_dir.join("libpace_runtime.a"),
            exe_dir.join("../lib/libpace_runtime.a"),
            exe_dir.join("pace_runtime.lib"),
            exe_dir.join("../lib/pace_runtime.lib"),
        ];

        if let Ok(cwd) = env::current_dir() {
            search_paths.push(cwd.join("target/debug/libpace_runtime.a"));
            search_paths.push(cwd.join("target/release/libpace_runtime.a"));
            search_paths.push(cwd.join("target/debug/pace_runtime.lib"));
            search_paths.push(cwd.join("target/release/pace_runtime.lib"));
            search_paths.push(cwd.join("compiler/runtime/target/debug/libpace_runtime.a"));
            search_paths.push(cwd.join("compiler/runtime/target/debug/pace_runtime.lib"));
        }
        
        let runtime_lib = search_paths.into_iter().find(|p| p.exists())
            .ok_or_else(|| "Runtime library (libpace_runtime.a) not found. Please ensure it is built and located alongside the pace executable or in the target directory.".to_string())?;

        // We will use the system `cc` to link the object file with our Rust staticlib
        // The Rust staticlib might require linking against standard system libraries (like pthread, dl, m)
        // but cc usually handles this natively.
        let output = Command::new("cc")
            .arg(object_file)
            .arg(&runtime_lib)
            .arg("-o")
            .arg(output_file)
            // .arg("-lpthread") // might be needed on some Linux setups for Rust std
            // .arg("-ldl")
            // .arg("-lm")
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    let os = std::env::consts::OS;
                    let install_msg = match os {
                        "linux" => "Please run 'sudo apt install build-essential' (Ubuntu/Debian) or 'sudo dnf install gcc' (Fedora) to install a C compiler.",
                        "macos" => "Please run 'xcode-select --install' in your terminal to install the Apple build tools.",
                        "windows" => "Please install MSVC Build Tools or MinGW to get a C compiler.",
                        _ => "Please install a C compiler (like gcc or clang) and ensure it is in your PATH.",
                    };
                    format!("Failed to invoke linker: 'cc' not found.\n{}", install_msg)
                } else {
                    format!("Failed to invoke linker: {}", e)
                }
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Linker failed:\n{}", stderr));
        }

        Ok(())
    }
}
