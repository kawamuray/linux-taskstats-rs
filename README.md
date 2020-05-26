linux-taskstats-rs
==================

Rust interface to [Linux's taskstats](https://www.kernel.org/doc/Documentation/accounting/taskstats.txt).

This crate provides access to taskstats which is known as a way to access task's "delay" information a.k.a [Delay Accounting](https://www.kernel.org/doc/html/latest/accounting/delay-accounting.html).


# Usage

```rust
use linux_taskstats::{self, Delays, Client};

fn get_pid_delays(pid: u32) -> Result<Delays, linux_taskstats::Error> {
    let client = Client::open()?;
    let ts = client.pid_stats(pid)?;
    ts.delays
}
```

# How to build

```sh
cargo test
cargo build
```

Or on platform other than linux:

```sh
./docker-build/build-docker-image.sh # Just once, creates a image `taskstats-build:latest`
./docker-build/build.sh
# The outputs will be created under docker-build/target
```

# License

[MIT](./LICENSE)
