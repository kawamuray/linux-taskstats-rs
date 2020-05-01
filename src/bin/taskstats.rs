use std::env;
use std::process;

use env_logger;
use taskstats::{self, Client, TaskStats};

fn print_stats(ts: &TaskStats) {
    println!("=== TID: {} ===", ts.tid);
    let cpu = ts.cpu;
    println!("--- CPU ---");
    println!("User Time (us): {}", cpu.utime_total.as_micros());
    println!("System Time (us): {}", cpu.stime_total.as_micros());
    println!("Real Time (us): {}", cpu.real_time_total.as_micros());
    println!("Virtual Time (us): {}", cpu.virtual_time_total.as_micros());
    let mem = ts.memory;
    println!("--- Memory ---");
    println!("RSS (MB-usec): {}", mem.rss_total);
    println!("Virtual (MB-usec): {}", mem.virt_total);
    println!(
        "Page Faults (minor:major): {}:{}",
        mem.minor_faults, mem.major_faults
    );
    let io = ts.io;
    println!("--- IO ---");
    println!("Read (bytes): {}", io.read_bytes);
    println!("Write (bytes): {}", io.write_bytes);
    println!(
        "Syscalls (read:write): {}:{}",
        io.read_syscalls, io.write_syscalls
    );
    let bio = ts.blkio;
    println!("--- Block Device IO ---");
    println!("Read (bytes): {}", bio.read_bytes);
    println!("Write (bytes): {}", bio.write_bytes);
    println!("Write Cancelled (bytes): {}", bio.cancelled_write_bytes);
    let cswt = ts.ctx_switches;
    println!("--- Context Switches ---");
    println!(
        "Voluntary:Non-voluntary: {}:{}",
        cswt.voluntary, cswt.non_voluntary
    );
    let delays = ts.delays;
    println!("--- Delays ---");
    println!(
        "CPU Total(nsec)/Count: {}/{}",
        delays.cpu.delay_total.as_nanos(),
        delays.cpu.count
    );
    println!(
        "BlkIO Total(nsec)/Count: {}/{}",
        delays.blkio.delay_total.as_nanos(),
        delays.blkio.count
    );
    println!(
        "SwapIn Total(nsec)/Count: {}/{}",
        delays.swapin.delay_total.as_nanos(),
        delays.swapin.count
    );
    println!(
        "Mem Reclaim Total(nsec)/Count: {}/{}",
        delays.freepages.delay_total.as_nanos(),
        delays.freepages.count
    );
}

fn main() {
    env_logger::init();

    let client = Client::open().expect("netlink init");
    for pid in env::args().into_iter().skip(1) {
        let pid = match pid.parse::<u32>() {
            Ok(pid) => pid,
            Err(_) => {
                eprintln!("Invalid PID: {}", pid);
                process::exit(1);
            }
        };

        let ts = client.pid_stats(pid).expect("get stats");
        print_stats(&ts);
    }
}
