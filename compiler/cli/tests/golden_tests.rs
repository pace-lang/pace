use std::process::Command;
use std::fs;
use std::path::{Path, PathBuf};

fn run_ui_test(file_path: &Path) {
    let mut cli_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    cli_path.push("../../target/debug/cli");

    // Ensure cli is built
    assert!(cli_path.exists(), "CLI executable not found at {:?}", cli_path);

    let output = Command::new(&cli_path)
        .arg("build")
        .arg(file_path)
        .output()
        .expect("Failed to execute cli command");

    // We only care about stderr for diagnostics tests
    let stderr = String::from_utf8(output.stderr).unwrap();
    
    // Normalize paths or variable memory addresses if needed, but for now just trim
    let normalized_stderr = stderr.trim().to_string();

    // Use insta to snapshot the stderr output
    let snapshot_name = file_path.file_name().unwrap().to_string_lossy().to_string().replace(".pace", "");
    
    let mut settings = insta::Settings::clone_current();
    settings.set_snapshot_path(file_path.parent().unwrap());
    settings.set_prepend_module_to_snapshot(false);
    
    settings.bind(|| {
        insta::assert_snapshot!(snapshot_name, normalized_stderr);
    });

    // Cleanup generated files
    let _ = std::fs::remove_file(file_path.with_extension("o"));
    let _ = std::fs::remove_file(file_path.with_extension(""));
}

#[test]
fn ui_tests() {
    let ui_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/ui");
    if !ui_dir.exists() {
        return;
    }

    // Recursively find all .pace files
    let mut files_to_test = Vec::new();
    let mut dirs_to_visit = vec![ui_dir];

    while let Some(dir) = dirs_to_visit.pop() {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                dirs_to_visit.push(path);
            } else if path.extension().and_then(|s| s.to_str()) == Some("pace") {
                files_to_test.push(path);
            }
        }
    }

    for file_path in files_to_test {
        run_ui_test(&file_path);
    }
}
