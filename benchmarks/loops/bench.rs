fn loop_sum() -> i64 {
    let mut sum: i64 = 0;
    let mut i: i64 = 0;
    while i < 10000000 {
        sum += i;
        i += 1;
    }
    sum
}
fn main() {
    println!("{}", loop_sum());
}
