use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::thread;
use std::time::Duration;

/// Do some I/O including fsync so this process would show some delays (delays.blkio)
/// by I/O and I/O activities (io.read_bytes, io.write_bytes ...)
fn main() {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open("/tmp/taskstats-tmp")
        .unwrap();
    let mut buf = [0u8; 100];

    for _ in 0..4000 {
        file.write(&buf).unwrap();
    }
    file.sync_data().unwrap();

    file.seek(SeekFrom::Start(0)).unwrap();
    for _ in 0..4000 {
        file.read(&mut buf).unwrap();
    }

    thread::sleep(Duration::from_secs(10));
}
