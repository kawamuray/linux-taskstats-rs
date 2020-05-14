use crate::format::{HeaderFormat, Printer};
use crate::Client;
use env_logger;
use std::io;

pub struct Config<H: HeaderFormat> {
    pub tids: Vec<u32>,
    pub verbose: bool,
    pub show_delays: bool,
    pub header_format: H,
}

pub fn taskstats_main<H: HeaderFormat>(config: Config<H>) {
    env_logger::init();

    let mut stats = Vec::new();
    let client = Client::open().expect("netlink init");
    for pid in config.tids {
        let ts = client.pid_stats(pid).expect("get stats");
        stats.push(ts);
    }

    let printer = Printer::new(config.header_format);

    let mut show_line = true;
    if config.verbose {
        printer
            .print_full(&mut io::stdout(), &stats)
            .expect("write stdout");
        show_line = false;
    }
    if config.show_delays {
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
