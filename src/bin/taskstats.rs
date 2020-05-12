use std::env;
use std::io;
use std::process;

use env_logger;
use taskstats::format::{DefaultHeaderFormat, Printer};
use taskstats::{self, Client};

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
        let printer = Printer::new(DefaultHeaderFormat::new());
        printer
            .print_full(&mut io::stdout(), &ts)
            .expect("write stdout")
    }
}
