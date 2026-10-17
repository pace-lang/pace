fn main() {
    println!("cargo:rerun-if-changed=src/c_utils.c");
    println!("cargo:rerun-if-changed=src/concurrency.c");
    println!("cargo:rerun-if-changed=src/collections.c");
    println!("cargo:rerun-if-changed=pace_runtime.h");
    
    cc::Build::new()
        .file("src/c_utils.c")
        .file("src/concurrency.c")
        .file("src/collections.c")
        .compile("c_utils");
}
