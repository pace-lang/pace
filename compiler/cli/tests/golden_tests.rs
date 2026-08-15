use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn run_ui_test(file_path: &Path) {
    if let Ok(content) = fs::read_to_string(file_path)
        && content.contains("// skip-test")
    {
        return;
    }

    let mut cli_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    cli_path.push("../../target/debug/cli");

    // Ensure cli is built
    assert!(
        cli_path.exists(),
        "CLI executable not found at {:?}",
        cli_path
    );

    let output = Command::new(&cli_path)
        .arg("run")
        .arg(file_path)
        .output()
        .expect("Failed to execute cli command");

    // We care about both stdout and stderr to verify compilation and execution
    let mut combined_output = String::from_utf8(output.stderr).unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    if !stdout.is_empty() {
        combined_output.push('\n');
        combined_output.push_str(&stdout);
    }

    // Determine the workspace root dynamically
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .to_string();

    // Normalize paths, variable memory addresses, and thread IDs in panics
    let normalized_stderr = combined_output
        .lines()
        .filter(|line| {
            !line.starts_with("Finished `dev` profile")
                && !line.starts_with("Finished `release` profile")
                && !line.starts_with("     Running `target/")
        })
        .map(|line| {
            if line.starts_with("thread '")
                && line.contains("panicked at")
                && let Some(idx) = line.find("panicked at")
            {
                return format!("thread 'main' {}", &line[idx..]);
            }
            line.replace(&workspace_root, "$WORKSPACE").to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();

    // Use insta to snapshot the stderr output
    let snapshot_name = file_path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string()
        .replace(".pace", "");

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
