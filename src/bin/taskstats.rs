use taskstats::cmd;
use taskstats::format::DefaultHeaderFormat;

fn main() {
    let app = cmd::taskstats_app();
    cmd::taskstats_main(&app.get_matches(), DefaultHeaderFormat::new())
}
