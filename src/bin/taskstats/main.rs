use clap::{Arg, ArgAction, Command};
use linux_taskstats::format::DefaultHeaderFormat;

mod cmd;

fn main() {
    let matches = Command::new("A command line interface to Linux taskstats")
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verfbose")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("show-delays")
                .short('d')
                .long("delay")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("TIDS")
                .index(1)
                .num_args(1..)
                .action(ArgAction::Append),
        )
        .get_matches();

    let tids: Vec<_> = matches
        .get_many::<u32>("TIDS")
        .unwrap()
        .map(|x| *x)
        .collect();

    let config = cmd::Config {
        tids,
        verbose: matches.contains_id("verbose"),
        show_delays: matches.contains_id("show-delays"),
        header_format: DefaultHeaderFormat::new(),
    };
    cmd::taskstats_main(config);
}
