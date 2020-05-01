use std::thread;
use std::time::Duration;

const NUM_THREADS: usize = 100_000;

/// Generate many threads so some of them are likely forced to wait for CPU (delays.cpu)
fn main() {
    let mut handles = Vec::with_capacity(NUM_THREADS);
    for _ in NUM_THREADS {
        let th = thread::spawn(|| {
            let mut n = 1;
            for _ in 0..1_000_000 {
                n = (n << 1) ^ 17;
            }
            // TODO: may need to make some side effect to avoid ellision
            thread::sleep(Duration::from_secs(10));
        });
        handles.push(th);
    }
    for th in handles {
        th.join().unwrap();
    }
}
