use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let runtime_dir = Path::new("../runtime").canonicalize().unwrap();

    // Rebuild if runtime C files change
    println!(
        "cargo:rerun-if-changed={}",
        runtime_dir.join("src").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        runtime_dir.join("Cargo.toml").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        runtime_dir.join("pace_runtime.c").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        runtime_dir.join("pace_runtime.h").display()
    );

    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());

    // Determine cargo arguments
    let mut args = vec!["build", "--manifest-path", "Cargo.toml"];
    if profile == "release" {
        args.push("--release");
    }

    let status = Command::new("cargo")
        .current_dir(&runtime_dir)
        .env("CARGO_TARGET_DIR", out_dir.clone() + "/runtime_target")
        .args(&args)
        .status()
        .expect("Failed to build pace-rt");

    if !status.success() {
        panic!("pace-rt build failed");
    }

    let target_dir = Path::new(&out_dir).join("runtime_target").join(&profile);

    // The staticlib name varies by platform.
    // On Linux/macOS it's libpace_rt.a, on Windows it's pace_rt.lib.
    let static_lib_name = if cfg!(windows) {
        "pace_rt.lib"
    } else {
        "libpace_rt.a"
    };

    let src_lib = target_dir.join(static_lib_name);
    let dest_lib = Path::new(&out_dir).join("libpace_rt.a"); // We always copy it as libpace_rt.a internally

    fs::copy(&src_lib, &dest_lib).unwrap_or_else(|e| {
        panic!(
            "Failed to copy {} to {}: {}",
            src_lib.display(),
            dest_lib.display(),
            e
        );
    });

    let src_header = runtime_dir.join("pace_runtime.h");
    let dest_header = Path::new(&out_dir).join("pace_runtime.h");
    fs::copy(&src_header, &dest_header).unwrap_or_else(|e| {
        panic!(
            "Failed to copy {} to {}: {}",
            src_header.display(),
            dest_header.display(),
            e
        );
    });
}
