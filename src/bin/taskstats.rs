use clap::{App, Arg};
use std::process;
use taskstats::cmd;
use taskstats::format::DefaultHeaderFormat;

fn main() {
    let matches = App::new("A command line interface to Linux taskstats")
        .arg(Arg::with_name("verbose").short("v").long("verbose"))
        .arg(Arg::with_name("show-delays").short("d").long("delay"))
        .arg(Arg::with_name("TIDS").index(1).multiple(true))
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
