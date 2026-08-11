use std::path::Path;
use std::process::Command;

pub struct Linker;

impl Linker {
    pub fn link(object_file: &Path, output_file: &Path) -> Result<(), String> {
        let runtime_src = include_str!("runtime.c");
        let runtime_file = output_file.with_extension("runtime.c");
        std::fs::write(&runtime_file, runtime_src)
            .map_err(|e| format!("Failed to write runtime file: {}", e))?;

        // We will use the system `cc` for the first implementation
        let output = Command::new("cc")
            .arg(object_file)
            .arg(&runtime_file)
            .arg("-o")
            .arg(output_file)
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    "Failed to invoke linker: 'cc' not found. Please install a C compiler (like gcc or clang) and ensure it's in your PATH.".to_string()
                } else {
                    format!("Failed to invoke linker: {}", e)
                }
            })?;

        let _ = std::fs::remove_file(&runtime_file);

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Linker failed:\n{}", stderr));
        }

        Ok(())
    }
}
