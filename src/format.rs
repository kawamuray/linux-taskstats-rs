use crate::TaskStats;
use prettytable::{self as ptable, cell, row};
use std::io::{self, Write};

pub trait HeaderFormat {
    fn format(&self, tid: u32) -> String;
}

#[derive(Default)]
pub struct DefaultHeaderFormat {}

impl DefaultHeaderFormat {
    pub fn new() -> Self {
        Default::default()
    }
}

impl HeaderFormat for DefaultHeaderFormat {
    fn format(&self, tid: u32) -> String {
        format!("TID: {}", tid)
    }
}

pub struct Printer<H: HeaderFormat> {
    header_format: H,
}

impl<H: HeaderFormat> Printer<H> {
    pub fn new(header_format: H) -> Self {
        Self { header_format }
    }

    pub fn print_summary_lines<W: Write>(
        &self,
        out: &mut W,
        stats: &[TaskStats],
    ) -> io::Result<()> {
        let mut table = ptable::Table::new();
        table.set_format(*ptable::format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
        table.add_row(row![
            c =>
            "Task",
            "utime",
            "stime",
            "rss",
            "vmem",
            "read",
            "write",
            "d:cpu",
            "d:bio",
            "d:swap",
            "d:reclaim"
        ]);
        for ts in stats {
            table.add_row(row![
                l->self.header_format.format(ts.tid),
                r->ts.cpu.utime_total.as_micros(),
                r->ts.cpu.stime_total.as_micros(),
                r->ts.memory.rss_total,
                r->ts.memory.virt_total,
                r->ts.io.read_bytes,
                r->ts.io.write_bytes,
                r->ts.delays.cpu.delay_total.as_nanos(),
                r->ts.delays.blkio.delay_total.as_nanos(),
                r->ts.delays.swapin.delay_total.as_nanos(),
                r->ts.delays.freepages.delay_total.as_nanos()
            ]);
        }
        table.print(out)?;
        Ok(())
    }

    pub fn print_delay_lines<W: Write>(&self, out: &mut W, stats: &[TaskStats]) -> io::Result<()> {
        let mut table = ptable::Table::new();
        table.set_format(*ptable::format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
        table.add_row(row![
            c =>
            "Task",
            "cpu avg",
            "blkio avg",
            "swapin avg",
            "reclaim avg",
            "cpu total",
            "blkio total",
            "swapin total",
            "reclaim total",
        ]);
        for ts in stats {
            let d = ts.delays;
            table.add_row(row![
                l->self.header_format.format(ts.tid),
                r->d.cpu.delay_total.as_nanos() as u64 / d.cpu.count.max(1),
                r->d.blkio.delay_total.as_nanos() as u64 / d.blkio.count.max(1),
                r->d.swapin.delay_total.as_nanos() as u64 / d.swapin.count.max(1),
                r->d.freepages.delay_total.as_nanos() as u64 / d.freepages.count.max(1),
                r->d.cpu.delay_total.as_nanos(),
                r->d.blkio.delay_total.as_nanos(),
                r->d.swapin.delay_total.as_nanos(),
                r->d.freepages.delay_total.as_nanos(),
            ]);
        }
        table.print(out)?;
        Ok(())
    }

    pub fn print_full<W: Write>(&self, out: &mut W, stats: &[TaskStats]) -> io::Result<()> {
        for ts in stats {
            writeln!(out, "=== {} ===", self.header_format.format(ts.tid))?;
            let cpu = ts.cpu;
            writeln!(out, "--- CPU ---")?;
            writeln!(out, "User Time (us): {}", cpu.utime_total.as_micros())?;
            writeln!(out, "System Time (us): {}", cpu.stime_total.as_micros())?;
            writeln!(out, "Real Time (us): {}", cpu.real_time_total.as_micros())?;
            writeln!(
                out,
                "Virtual Time (us): {}",
                cpu.virtual_time_total.as_micros()
            )?;
            let mem = ts.memory;
            writeln!(out, "--- Memory ---")?;
            writeln!(out, "RSS (MB-usec): {}", mem.rss_total)?;
            writeln!(out, "Virtual (MB-usec): {}", mem.virt_total)?;
            writeln!(
                out,
                "Page Faults (minor:major): {}:{}",
                mem.minor_faults, mem.major_faults
            )?;
            let io = ts.io;
            writeln!(out, "--- IO ---")?;
            writeln!(out, "Read (bytes): {}", io.read_bytes)?;
            writeln!(out, "Write (bytes): {}", io.write_bytes)?;
            writeln!(
                out,
                "Syscalls (read:write): {}:{}",
                io.read_syscalls, io.write_syscalls
            )?;
            let bio = ts.blkio;
            writeln!(out, "--- Block Device IO ---")?;
            writeln!(out, "Read (bytes): {}", bio.read_bytes)?;
            writeln!(out, "Write (bytes): {}", bio.write_bytes)?;
            writeln!(
                out,
                "Write Cancelled (bytes): {}",
                bio.cancelled_write_bytes
            )?;
            let cswt = ts.ctx_switches;
            writeln!(out, "--- Context Switches ---")?;
            writeln!(
                out,
                "Voluntary:Non-voluntary: {}:{}",
                cswt.voluntary, cswt.non_voluntary
            )?;
            let delays = ts.delays;
            writeln!(out, "--- Delays ---")?;
            writeln!(
                out,
                "CPU Total(nsec)/Count: {}/{}",
                delays.cpu.delay_total.as_nanos(),
                delays.cpu.count
            )?;
            writeln!(
                out,
                "BlkIO Total(nsec)/Count: {}/{}",
                delays.blkio.delay_total.as_nanos(),
                delays.blkio.count
            )?;
            writeln!(
                out,
                "SwapIn Total(nsec)/Count: {}/{}",
                delays.swapin.delay_total.as_nanos(),
                delays.swapin.count
            )?;
            writeln!(
                out,
                "Mem Reclaim Total(nsec)/Count: {}/{}",
                delays.freepages.delay_total.as_nanos(),
                delays.freepages.count
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    use std::time::Duration;

    const TS: TaskStats = TaskStats {
        tid: 1234,
        cpu: Cpu {
            utime_total: Duration::from_micros(12),
            stime_total: Duration::from_micros(34),
            real_time_total: Duration::from_micros(56),
            virtual_time_total: Duration::from_micros(78),
        },
        memory: Memory {
            rss_total: 12,
            virt_total: 34,
            minor_faults: 56,
            major_faults: 78,
        },
        io: Io {
            read_bytes: 12,
            write_bytes: 34,
            read_syscalls: 56,
            write_syscalls: 78,
        },
        blkio: BlkIo {
            read_bytes: 12,
            write_bytes: 34,
            cancelled_write_bytes: 56,
        },
        ctx_switches: ContextSwitches {
            voluntary: 12,
            non_voluntary: 34,
        },
        delays: Delays {
            cpu: DelayStat {
                count: 12,
                delay_total: Duration::from_nanos(34),
            },
            blkio: DelayStat {
                count: 56,
                delay_total: Duration::from_nanos(78),
            },
            swapin: DelayStat {
                count: 123,
                delay_total: Duration::from_nanos(456),
            },
            freepages: DelayStat {
                count: 789,
                delay_total: Duration::from_nanos(1234),
            },
        },
        inner_buf: [0u8; TASKSTATS_SIZE],
    };

    #[test]
    fn test_print_summary_lines() {
        let expect =
            "   Task    | utime | stime | rss | vmem | read | write | d:cpu | d:bio | d:swap | d:reclaim 
 TID: 1234 |    12 |    34 |  12 |   34 |   12 |    34 |    34 |    78 |    456 |      1234 
 TID: 1234 |    12 |    34 |  12 |   34 |   12 |    34 |    34 |    78 |    456 |      1234 
";

        let printer = Printer::new(DefaultHeaderFormat::new());
        let mut out = Vec::new();
        printer.print_summary_lines(&mut out, &[TS, TS]).unwrap();
        assert_eq!(expect, String::from_utf8(out).unwrap());
    }

    #[test]
    fn test_print_delay_lines() {
        let expect = "   Task    | cpu avg | blkio avg | swapin avg | reclaim avg | cpu total | blkio total | swapin total | reclaim total 
 TID: 1234 |       2 |         1 |          3 |           1 |        34 |          78 |          456 |          1234 
 TID: 1234 |       2 |         1 |          3 |           1 |        34 |          78 |          456 |          1234 
";

        let printer = Printer::new(DefaultHeaderFormat::new());
        let mut out = Vec::new();
        printer.print_delay_lines(&mut out, &[TS, TS]).unwrap();
        assert_eq!(expect, String::from_utf8(out).unwrap());
    }

    #[test]
    fn test_print_full() {
        let expect = "=== TID: 1234 ===
--- CPU ---
User Time (us): 12
System Time (us): 34
Real Time (us): 56
Virtual Time (us): 78
--- Memory ---
RSS (MB-usec): 12
Virtual (MB-usec): 34
Page Faults (minor:major): 56:78
--- IO ---
Read (bytes): 12
Write (bytes): 34
Syscalls (read:write): 56:78
--- Block Device IO ---
Read (bytes): 12
Write (bytes): 34
Write Cancelled (bytes): 56
--- Context Switches ---
Voluntary:Non-voluntary: 12:34
--- Delays ---
CPU Total(nsec)/Count: 34/12
BlkIO Total(nsec)/Count: 78/56
SwapIn Total(nsec)/Count: 456/123
Mem Reclaim Total(nsec)/Count: 1234/789
";

        let printer = Printer::new(DefaultHeaderFormat::new());
        let mut out = Vec::new();
        printer.print_full(&mut out, &[TS]).unwrap();
        assert_eq!(expect, String::from_utf8(out).unwrap());
    }
}
