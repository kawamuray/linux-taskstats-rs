use crate::format::{HeaderFormat, Printer};
use crate::Client;
use clap::{App, Arg, ArgMatches};
use env_logger;
use std::io;
use std::process;

pub fn taskstats_app<'a, 'b>() -> App<'a, 'b> {
    App::new("A command line interface to Linux taskstats")
        .arg(Arg::with_name("verbose").short("v").long("verbose"))
        .arg(Arg::with_name("show-delays").short("d").long("delay"))
        .arg(Arg::with_name("TIDS").index(1).multiple(true))
}

pub fn taskstats_main<H: HeaderFormat>(matches: &ArgMatches, header_format: H) {
    env_logger::init();

    let mut stats = Vec::new();
    let client = Client::open().expect("netlink init");
    for pid in matches.values_of("TIDS").unwrap() {
        let pid = match pid.parse::<u32>() {
            Ok(pid) => pid,
            Err(_) => {
                eprintln!("Invalid PID: {}", pid);
                process::exit(1);
            }
        };
        let ts = client.pid_stats(pid).expect("get stats");
        stats.push(ts);
    }

    let printer = Printer::new(header_format);

    let mut show_line = true;
    if matches.is_present("verbose") {
        printer
            .print_full(&mut io::stdout(), &stats)
            .expect("write stdout");
        show_line = false;
    }
    if matches.is_present("show-delays") {
        printer
            .print_delay_lines(&mut io::stdout(), &stats)
            .expect("write stdout");
        show_line = false;
    }

    if show_line {
        printer
            .print_summary_lines(&mut io::stdout(), &stats)
            .expect("write stdout")
    }
}
