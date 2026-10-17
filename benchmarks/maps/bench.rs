use std::collections::HashMap;
fn map_test() -> i64 {
    let mut map = HashMap::new();
    let mut i = 0;
    while i < 10000 {
        map.insert(i, i * 2);
        i += 1;
    }
    
    let mut sum = 0;
    i = 0;
    while i < 10000 {
        sum += map.get(&i).unwrap();
        i += 1;
    }
    sum
}
fn main() {
    println!("{}", map_test());
}
