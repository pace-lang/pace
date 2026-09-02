struct Person {
    age: i64,
    weight: i64,
}

impl Person {
    fn new(age: i64, weight: i64) -> Self {
        Self { age, weight }
    }
    fn get_value(&self) -> i64 {
        self.age + self.weight
    }
}

fn main() {
    let mut sum: i64 = 0;
    for i in 0..1000000 {
        let p = Person::new(i, i + 1);
        sum += p.get_value();
    }
    println!("{}", sum);
}
