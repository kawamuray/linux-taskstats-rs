#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

#[allow(dead_code)]
mod c_headers;
#[cfg(feature = "format")]
pub mod format;
mod model;
pub(crate) mod netlink;
pub use model::*;

pub use c_headers::taskstats as __bindgen_taskstats;
use c_headers::{
    __u16, __u32, __u64, __u8, TASKSTATS_CMD_ATTR_PID, TASKSTATS_CMD_ATTR_TGID, TASKSTATS_CMD_GET,
    TASKSTATS_GENL_NAME, TASKSTATS_TYPE_AGGR_PID, TASKSTATS_TYPE_AGGR_TGID, TASKSTATS_TYPE_NULL,
    TASKSTATS_TYPE_PID, TASKSTATS_TYPE_STATS, TASKSTATS_TYPE_TGID,
};
use log::{debug, warn};
use netlink::Netlink;
use netlink::NlPayload;
use std::mem;
use std::slice;
use thiserror::Error;

/// Errors possibly returned by `Client`
#[derive(Debug, Error)]
pub enum Error {
    /// Error in netlink socket/protocol layer
    #[error("error in netlink communication with kernel: {0}")]
    Netlink(#[from] netlink::Error),
    /// Failed to lookup family ID for taskstats
    #[error("no family id corresponding to taskstats found")]
    NoFamilyId,
    /// Any unknown error
    #[error("unknown error: {0}")]
    Unknown(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Interface to access kernel taskstats API through the netlink socket.
pub struct Client {
    netlink: Netlink,
    ts_family_id: u16,
}

impl Client {
    /// Open netlink socket against kernel and create a new instance of `Client`
    ///
    /// # Errors
    /// * when netlink socket initialization failed
    /// * when kernel doesn't offer family id for taskstats
    pub fn open() -> Result<Self> {
        let netlink = Netlink::open()?;
        let ts_family_id = Self::lookup_family_id(&netlink)?;
        debug!("Found taskstats family id: {}", ts_family_id);
        Ok(Self {
            netlink,
            ts_family_id,
        })
    }

    fn lookup_family_id(netlink: &Netlink) -> Result<u16> {
        netlink.send_cmd(
            libc::GENL_ID_CTRL as u16,
            libc::CTRL_CMD_GETFAMILY as u8,
            libc::CTRL_ATTR_FAMILY_NAME as u16,
            TASKSTATS_GENL_NAME,
        )?;

        let resp = netlink.recv_response()?;
        for na in resp.payload_as_nlattrs() {
            debug!("Family lookup: got nla_type: {}", na.header.nla_type);
            if na.header.nla_type == libc::CTRL_ATTR_FAMILY_ID as u16 {
                return Ok(*na.payload_as());
            }
        }
        Err(Error::NoFamilyId)
    }

    /// Obtain taskstats for given task ID (e.g. single thread of a multithreaded process)
    ///
    /// # Arguments
    /// * `tid` - Kernel task ID ("pid", "tid" and "task" are used interchangeably and refer to the
    ///   standard Linux task defined by struct task_struct)
    ///
    /// # Return
    /// * `TaskStats` storing the target task's stats
    ///
    /// # Errors
    /// * when netlink socket failed
    /// * when kernel responded error
    /// * when the returned data couldn't be interpreted
    pub fn pid_stats(&self, tid: u32) -> Result<TaskStats> {
        self.netlink.send_cmd(
            self.ts_family_id,
            TASKSTATS_CMD_GET as u8,
            TASKSTATS_CMD_ATTR_PID as u16,
            tid.as_buf(),
        )?;

        let resp = self.netlink.recv_response()?;
        for na in resp.payload_as_nlattrs() {
            match na.header.nla_type as u32 {
                TASKSTATS_TYPE_NULL => break,
                TASKSTATS_TYPE_AGGR_PID => {
                    for inner in na.payload_as_nlattrs() {
                        match inner.header.nla_type as u32 {
                            TASKSTATS_TYPE_PID => debug!("Received TASKSTATS_TYPE_PID"),
                            TASKSTATS_TYPE_TGID => warn!("Received TASKSTATS_TYPE_TGID"),
                            TASKSTATS_TYPE_STATS => {
                                return Ok(TaskStats::from(inner.payload()));
                            }
                            unknown => warn!("Skipping unknown nla_type: {}", unknown),
                        }
                    }
                }
                unknown => warn!("Skipping unknown nla_type: {}", unknown),
            }
        }
        Err(Error::Unknown(
            "no TASKSTATS_TYPE_STATS found in response".to_string(),
        ))
    }

    /// Obtain taskstats for given thread group ID (e.g. cumulated statistics of a multithreaded process)
    ///
    /// # Arguments
    /// * `tgid` - Kernel thread group ID ("tgid", "process" and "thread group" are used
    ///   interchangeably and refer to the traditional Unix process)
    ///
    /// # Return
    /// * `TaskStats` storing the target thread group's aggregated stats
    ///
    /// # Errors
    /// * when netlink socket failed
    /// * when kernel responded error
    /// * when the returned data couldn't be interpreted
    pub fn tgid_stats(&self, tgid: u32) -> Result<TaskStats> {
        self.netlink.send_cmd(
            self.ts_family_id,
            TASKSTATS_CMD_GET as u8,
            TASKSTATS_CMD_ATTR_TGID as u16,
            tgid.as_buf(),
        )?;

        let resp = self.netlink.recv_response()?;
        for na in resp.payload_as_nlattrs() {
            match na.header.nla_type as u32 {
                TASKSTATS_TYPE_NULL => break,
                TASKSTATS_TYPE_AGGR_TGID => {
                    for inner in na.payload_as_nlattrs() {
                        match inner.header.nla_type as u32 {
                            TASKSTATS_TYPE_PID => warn!("Received TASKSTATS_TYPE_PID"),
                            TASKSTATS_TYPE_TGID => debug!("Received TASKSTATS_TYPE_TGID"),
                            TASKSTATS_TYPE_STATS => {
                                return Ok(TaskStats::from(inner.payload()));
                            }
                            unknown => warn!("Skipping unknown nla_type: {}", unknown),
                        }
                    }
                }
                unknown => warn!("Skipping unknown nla_type: {}", unknown),
            }
        }
        Err(Error::Unknown(
            "no TASKSTATS_TYPE_STATS found in response".to_string(),
        ))
    }
}

trait AsBuf<T> {
    fn as_buf(&self) -> &[u8];

    fn as_buf_mut(&mut self) -> &mut [u8];
}

impl<T> AsBuf<T> for T {
    #[inline]
    fn as_buf(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self as *const T as *const u8, mem::size_of::<T>()) }
    }

    #[inline]
    fn as_buf_mut(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self as *mut T as *mut u8, mem::size_of::<T>()) }
    }
}

/// This is custom copy of the generated `struct taskstats` from linux version 3.10.0 and in this crate
/// this type is used to read binary data transferred from linux kernel.
/// The reason of doing this despite the `bindgen` generates rust bindings including `struct taskstats`
/// is due to potential corruption of on-memory struct layout likely caused by old clang version.
/// ref: https://github.com/rust-lang/rust-bindgen/issues/867
/// Specifically, when `bindgen` generates `struct taskstats` with older clang version, the resulting
/// struct defined in generated rust source code contains size padding as the last member of the struct
/// causing offset of members after `ac_uid` to shift 4-byte or more and the result data becomes unreliable.
/// The `bindgen` generated definition works well in environment that uses newer clang versions, but I
/// decided to use copied definition of this struct for the time being by following considerations:
/// * The struct definition rarely evolves.
/// * Returning corrupted data silently is critical and much worse than not providing from the beginning.
/// * If user of this crate still needs to access the exactly original definition generated by `bindgen`,
///   it might still be possible by casting type to `__bindgen_taskstats` exported by this crate.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct taskstats {
    pub version: __u16,
    pub ac_exitcode: __u32,
    pub ac_flag: __u8,
    pub ac_nice: __u8,
    pub cpu_count: __u64,
    pub cpu_delay_total: __u64,
    pub blkio_count: __u64,
    pub blkio_delay_total: __u64,
    pub swapin_count: __u64,
    pub swapin_delay_total: __u64,
    pub cpu_run_real_total: __u64,
    pub cpu_run_virtual_total: __u64,
    pub ac_comm: [::std::os::raw::c_char; 32usize],
    pub ac_sched: __u8,
    pub ac_pad: [__u8; 3usize],
    pub __unused_padding: u32,
    pub ac_uid: __u32,
    pub ac_gid: __u32,
    pub ac_pid: __u32,
    pub ac_ppid: __u32,
    pub ac_btime: __u32,
    pub ac_etime: __u64,
    pub ac_utime: __u64,
    pub ac_stime: __u64,
    pub ac_minflt: __u64,
    pub ac_majflt: __u64,
    pub coremem: __u64,
    pub virtmem: __u64,
    pub hiwater_rss: __u64,
    pub hiwater_vm: __u64,
    pub read_char: __u64,
    pub write_char: __u64,
    pub read_syscalls: __u64,
    pub write_syscalls: __u64,
    pub read_bytes: __u64,
    pub write_bytes: __u64,
    pub cancelled_write_bytes: __u64,
    pub nvcsw: __u64,
    pub nivcsw: __u64,
    pub ac_utimescaled: __u64,
    pub ac_stimescaled: __u64,
    pub cpu_scaled_run_real_total: __u64,
    pub freepages_count: __u64,
    pub freepages_delay_total: __u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(test_priv)]
    #[test]
    fn test_pid_stats() {
        let client = Client::open().unwrap();
        let ts = client.pid_stats(std::process::id()).unwrap();

        // Just asserts some fields which do likely have positive values
        assert!(ts.cpu.utime_total.as_nanos() > 0);
        assert!(ts.memory.rss_total > 0);
    }

    #[cfg(test_priv)]
    #[test]
    fn test_tgid_stats() {
        let client = Client::open().unwrap();
        let ts = client.tgid_stats(std::process::id()).unwrap();

        // Just asserts some fields which do likely have positive values
        assert!(ts.cpu.utime_total.as_nanos() > 0);
        assert!(ts.memory.rss_total > 0);
    }

    #[test]
    fn test_struct_taskstats_alignment() {
        // Automatically generated by tools/gen_layout_test.sh
        assert_eq!(328, std::mem::size_of::<taskstats>());
        assert_eq!(0, unsafe {
            &(*(std::ptr::null::<taskstats>())).version as *const _ as usize
        });
        assert_eq!(4, unsafe {
            &(*(std::ptr::null::<taskstats>())).ac_exitcode as *const _ as usize
        });
        assert_eq!(8, unsafe {
            &(*(std::ptr::null::<taskstats>())).ac_flag as *const _ as usize
        });
        assert_eq!(9, unsafe {
            &(*(std::ptr::null::<taskstats>())).ac_nice as *const _ as usize
        });
        assert_eq!(16, unsafe {
            &(*(std::ptr::null::<taskstats>())).cpu_count as *const _ as usize
        });
        assert_eq!(24, unsafe {
            &(*(std::ptr::null::<taskstats>())).cpu_delay_total as *const _ as usize
        });
        assert_eq!(32, unsafe {
            &(*(std::ptr::null::<taskstats>())).blkio_count as *const _ as usize
        });
        assert_eq!(40, unsafe {
            &(*(std::ptr::null::<taskstats>())).blkio_delay_total as *const _ as usize
        });
        assert_eq!(48, unsafe {
            &(*(std::ptr::null::<taskstats>())).swapin_count as *const _ as usize
        });
        assert_eq!(56, unsafe {
            &(*(std::ptr::null::<taskstats>())).swapin_delay_total as *const _ as usize
        });
        assert_eq!(64, unsafe {
            &(*(std::ptr::null::<taskstats>())).cpu_run_real_total as *const _ as usize
        });
        assert_eq!(72, unsafe {
            &(*(std::ptr::null::<taskstats>())).cpu_run_virtual_total as *const _ as usize
        });
        assert_eq!(80, unsafe {
            &(*(std::ptr::null::<taskstats>())).ac_comm as *const _ as usize
        });
        assert_eq!(112, unsafe {
            &(*(std::ptr::null::<taskstats>())).ac_sched as *const _ as usize
        });
        assert_eq!(113, unsafe {
            &(*(std::ptr::null::<taskstats>())).ac_pad as *const _ as usize
        });
        assert_eq!(120, unsafe {
            &(*(std::ptr::null::<taskstats>())).ac_uid as *const _ as usize
        });
        assert_eq!(124, unsafe {
            &(*(std::ptr::null::<taskstats>())).ac_gid as *const _ as usize
        });
        assert_eq!(128, unsafe {
            &(*(std::ptr::null::<taskstats>())).ac_pid as *const _ as usize
        });
        assert_eq!(132, unsafe {
            &(*(std::ptr::null::<taskstats>())).ac_ppid as *const _ as usize
        });
        assert_eq!(136, unsafe {
            &(*(std::ptr::null::<taskstats>())).ac_btime as *const _ as usize
        });
        assert_eq!(144, unsafe {
            &(*(std::ptr::null::<taskstats>())).ac_etime as *const _ as usize
        });
        assert_eq!(152, unsafe {
            &(*(std::ptr::null::<taskstats>())).ac_utime as *const _ as usize
        });
        assert_eq!(160, unsafe {
            &(*(std::ptr::null::<taskstats>())).ac_stime as *const _ as usize
        });
        assert_eq!(168, unsafe {
            &(*(std::ptr::null::<taskstats>())).ac_minflt as *const _ as usize
        });
        assert_eq!(176, unsafe {
            &(*(std::ptr::null::<taskstats>())).ac_majflt as *const _ as usize
        });
        assert_eq!(184, unsafe {
            &(*(std::ptr::null::<taskstats>())).coremem as *const _ as usize
        });
        assert_eq!(192, unsafe {
            &(*(std::ptr::null::<taskstats>())).virtmem as *const _ as usize
        });
        assert_eq!(200, unsafe {
            &(*(std::ptr::null::<taskstats>())).hiwater_rss as *const _ as usize
        });
        assert_eq!(208, unsafe {
            &(*(std::ptr::null::<taskstats>())).hiwater_vm as *const _ as usize
        });
        assert_eq!(216, unsafe {
            &(*(std::ptr::null::<taskstats>())).read_char as *const _ as usize
        });
        assert_eq!(224, unsafe {
            &(*(std::ptr::null::<taskstats>())).write_char as *const _ as usize
        });
        assert_eq!(232, unsafe {
            &(*(std::ptr::null::<taskstats>())).read_syscalls as *const _ as usize
        });
        assert_eq!(240, unsafe {
            &(*(std::ptr::null::<taskstats>())).write_syscalls as *const _ as usize
        });
        assert_eq!(248, unsafe {
            &(*(std::ptr::null::<taskstats>())).read_bytes as *const _ as usize
        });
        assert_eq!(256, unsafe {
            &(*(std::ptr::null::<taskstats>())).write_bytes as *const _ as usize
        });
        assert_eq!(264, unsafe {
            &(*(std::ptr::null::<taskstats>())).cancelled_write_bytes as *const _ as usize
        });
        assert_eq!(272, unsafe {
            &(*(std::ptr::null::<taskstats>())).nvcsw as *const _ as usize
        });
        assert_eq!(280, unsafe {
            &(*(std::ptr::null::<taskstats>())).nivcsw as *const _ as usize
        });
        assert_eq!(288, unsafe {
            &(*(std::ptr::null::<taskstats>())).ac_utimescaled as *const _ as usize
        });
        assert_eq!(296, unsafe {
            &(*(std::ptr::null::<taskstats>())).ac_stimescaled as *const _ as usize
        });
        assert_eq!(304, unsafe {
            &(*(std::ptr::null::<taskstats>())).cpu_scaled_run_real_total as *const _ as usize
        });
        assert_eq!(312, unsafe {
            &(*(std::ptr::null::<taskstats>())).freepages_count as *const _ as usize
        });
        assert_eq!(320, unsafe {
            &(*(std::ptr::null::<taskstats>())).freepages_delay_total as *const _ as usize
        });
    }
}
