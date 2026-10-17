fn concat_test() -> String {
    let mut s = String::new();
    let mut i = 0;
    while i < 10000 {
        s.push_str("a");
        i += 1;
    }
    s
}
fn main() {
    println!("{}", concat_test().len());
}
