fn fib(n: i32) -> i32 {
    if n <= 1 {
        return n;
    }
    fib(n - 1) + fib(n - 2)
}

fn main() {
    println!("Calculating fib(35)...");
    let result = fib(35);
    println!("Result:\n{}", result);
}
