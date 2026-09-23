fn main() {
    cc::Build::new()
        .file("src/c_utils.c")
        .file("src/concurrency.c")
        .compile("c_utils");
}
