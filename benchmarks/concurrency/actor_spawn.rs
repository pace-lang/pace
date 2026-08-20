use std::sync::mpsc;
use std::thread;

fn main() {
    let mut handles = Vec::new();
    for i in 0..10000 {
        handles.push(thread::spawn(move || {
            let _id = i;
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    println!("Spawned 10000 actors.");
}
