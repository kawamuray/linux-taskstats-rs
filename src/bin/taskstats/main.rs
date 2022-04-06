use clap::{Arg, Command};
use linux_taskstats::format::DefaultHeaderFormat;
use std::process;

mod cmd;

fn main() {
    let matches = Command::new("A command line interface to Linux taskstats")
        .arg(Arg::new("verbose").short('v').long("verbose"))
        .arg(Arg::new("show-delays").short('d').long("delay"))
        .arg(Arg::new("TIDS").index(1).multiple_values(true))
        .get_matches();

    let tids: Vec<_> = matches
        .values_of("TIDS")
        .unwrap()
        .map(|v| match v.parse::<u32>() {
            Ok(pid) => pid,
            Err(_) => {
                eprintln!("Invalid PID: {}", v);
                process::exit(1);
            }
        })
        .collect();

    let config = cmd::Config {
        tids,
        verbose: matches.is_present("verbose"),
        show_delays: matches.is_present("show-delays"),
        header_format: DefaultHeaderFormat::new(),
    };
    cmd::taskstats_main(config);
}
