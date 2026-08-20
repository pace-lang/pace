fn main() {
    let mut s = String::new();
    for _ in 0..100000 {
        s.push_str("a");
    }
    println!("{}", s.len());
}
