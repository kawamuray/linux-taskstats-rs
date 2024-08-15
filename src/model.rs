use crate::c_headers;
use crate::taskstats;
use std::mem;
use std::time::Duration;

// https://stackoverflow.com/questions/53619695/calculating-maximum-value-of-a-set-of-constant-expressions-at-compile-time
const fn const_max(a: usize, b: usize) -> usize {
    [a, b][(a < b) as usize]
}

pub const TASKSTATS_SIZE: usize = const_max(
    mem::size_of::<taskstats>(),
    mem::size_of::<c_headers::taskstats>(),
);

/// The taskstats representation for a task.
/// This struct remaps commonly used `struct taskstats` fields for primarily:
/// * Access values with rust's primitive types
/// * Better structured organization of group of fields
/// * Support serialization
///
/// There are more (but may not much interested) fields in the original
/// `struct taskstats` and they are accessible through obtaining the original
/// struct by `TaskStats#inner()`.
#[derive(Clone, Copy, Debug)]
pub struct TaskStats {
    pub(crate) inner_buf: [u8; TASKSTATS_SIZE],
    /// The target task ID
    pub tid: u32,
    /// Staticstics related to CPU time
    pub cpu: Cpu,
    /// Statistics related to memory, vm
    pub memory: Memory,
    /// Staticstics related to I/O at syscall surface
    pub io: Io,
    /// Statistics related to I/O at block device level
    pub blkio: BlkIo,
    /// Statistics related to context switches
    pub ctx_switches: ContextSwitches,
    /// Statistics related to scheduling delay (delay accounting)
    pub delays: Delays,
}

/// Staticstics related to CPU time
#[derive(Debug, Clone, Copy)]
pub struct Cpu {
    /// User CPU time
    pub utime_total: Duration,
    /// System CPU time
    pub stime_total: Duration,
    /// Wall-clock running time
    pub real_time_total: Duration,
    /// Virtual running time
    pub virtual_time_total: Duration,
}

/// Statistics related to memory, vm
#[derive(Debug, Clone, Copy)]
pub struct Memory {
    /// Accumulated RSS usage in duration of a task, in MBytes-usecs
    pub rss_total: u64,
    /// Accumulated virtual memory usage in duration of a task
    pub virt_total: u64,
    /// Minor faults count
    pub minor_faults: u64,
    /// Major faults count
    pub major_faults: u64,
}

/// Staticstics related to I/O at syscall surface
#[derive(Debug, Clone, Copy)]
pub struct Io {
    /// Bytes read
    pub read_bytes: u64,
    /// Bytes written
    pub write_bytes: u64,
    /// Number of read syscalls
    pub read_syscalls: u64,
    /// Number of write syscalls
    pub write_syscalls: u64,
}

/// Statistics related to I/O at block device level
#[derive(Debug, Clone, Copy)]
pub struct BlkIo {
    /// Bytes read
    pub read_bytes: u64,
    /// Bytes written
    pub write_bytes: u64,
    /// Bytes of cancelled writes
    pub cancelled_write_bytes: u64,
}

/// Statistics related to context switches
#[derive(Debug, Clone, Copy)]
pub struct ContextSwitches {
    /// Count of voluntary context switches
    pub voluntary: u64,
    /// Count of non-voluntary context switches
    pub non_voluntary: u64,
}

/// Statistics related to scheduling delay (delay accounting)
#[derive(Debug, Clone, Copy)]
pub struct Delays {
    /// Delay waiting for cpu, while runnable
    pub cpu: DelayStat,
    /// Delay waiting for synchronous block I/O to complete
    pub blkio: DelayStat,
    /// Delay waiting for page fault I/O (swap in only)
    pub swapin: DelayStat,
    /// Delay waiting for memory reclaim
    pub freepages: DelayStat,
}

#[derive(Debug, Clone, Copy)]
pub struct DelayStat {
    /// Number of delay values recorded
    pub count: u64,
    /// Cumulative total delay
    pub delay_total: Duration,
}

impl From<&[u8]> for TaskStats {
    fn from(buf: &[u8]) -> Self {
        let mut inner_buf = [0u8; TASKSTATS_SIZE];
        inner_buf.copy_from_slice(&buf[..TASKSTATS_SIZE]);
        let ts = unsafe { &*(inner_buf.as_ptr() as *const _ as *const taskstats) };
        TaskStats {
            tid: ts.ac_pid,
            cpu: Cpu {
                utime_total: Duration::from_micros(ts.ac_utime),
                stime_total: Duration::from_micros(ts.ac_stime),
                real_time_total: Duration::from_nanos(ts.cpu_run_real_total),
                virtual_time_total: Duration::from_nanos(ts.cpu_run_virtual_total),
            },
            memory: Memory {
                rss_total: ts.coremem,
                virt_total: ts.virtmem,
                minor_faults: ts.ac_minflt,
                major_faults: ts.ac_majflt,
            },
            io: Io {
                read_bytes: ts.read_char,
                write_bytes: ts.write_char,
                read_syscalls: ts.read_syscalls,
                write_syscalls: ts.write_syscalls,
            },
            blkio: BlkIo {
                read_bytes: ts.read_bytes,
                write_bytes: ts.write_bytes,
                cancelled_write_bytes: ts.cancelled_write_bytes,
            },
            ctx_switches: ContextSwitches {
                voluntary: ts.nvcsw,
                non_voluntary: ts.nivcsw,
            },
            delays: Delays {
                cpu: DelayStat {
                    count: ts.cpu_count,
                    delay_total: Duration::from_nanos(ts.cpu_delay_total),
                },
                blkio: DelayStat {
                    count: ts.blkio_count,
                    delay_total: Duration::from_nanos(ts.blkio_delay_total),
                },
                swapin: DelayStat {
                    count: ts.swapin_count,
                    delay_total: Duration::from_nanos(ts.swapin_delay_total),
                },
                freepages: DelayStat {
                    count: ts.freepages_count,
                    delay_total: Duration::from_nanos(ts.freepages_delay_total),
                },
            },
            inner_buf,
        }
    }
}

impl TaskStats {
    /// Return inner representation of taskstats.
    ///
    /// The returned value is an instance of `struct taskstats` that was
    /// received from kernel.
    /// `TaskStats` remaps most of its fields into rust-friendly types and
    /// structure, so this inner object should be referred only when the user
    /// wants to access more information than available in remapped fields.
    pub fn inner(&self) -> &taskstats {
        unsafe { &*(self.inner_buf.as_ptr() as *const _ as *const taskstats) }
    }
}
