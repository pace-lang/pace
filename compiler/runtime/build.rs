fn main() {
    cc::Build::new()
        .file("src/c_utils.c")
        .compile("c_utils");
}
