fn main() {
    println!("cargo:rerun-if-changed=src/c_utils.c");
    println!("cargo:rerun-if-changed=src/concurrency.c");
    println!("cargo:rerun-if-changed=src/collections_common.h");
    println!("cargo:rerun-if-changed=src/list.c");
    println!("cargo:rerun-if-changed=src/map.c");
    println!("cargo:rerun-if-changed=src/set.c");
    println!("cargo:rerun-if-changed=pace_runtime.h");
    
    cc::Build::new()
        .file("src/c_utils.c")
        .file("src/concurrency.c")
        .file("src/list.c")
        .file("src/map.c")
        .file("src/set.c")
        .file("src/strings.c")
        .file("src/stdlib.c")
        .compile("c_utils");
}
