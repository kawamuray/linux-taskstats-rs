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

pub use c_headers::taskstats;
use c_headers::{
    __u16, __u32, __u64, __u8, TASKSTATS_CMD_ATTR_DEREGISTER_CPUMASK, TASKSTATS_CMD_ATTR_PID,
    TASKSTATS_CMD_ATTR_REGISTER_CPUMASK, TASKSTATS_CMD_ATTR_TGID, TASKSTATS_CMD_GET,
    TASKSTATS_GENL_NAME, TASKSTATS_TYPE_AGGR_PID, TASKSTATS_TYPE_AGGR_TGID, TASKSTATS_TYPE_NULL,
    TASKSTATS_TYPE_PID, TASKSTATS_TYPE_STATS, TASKSTATS_TYPE_TGID,
};
use log::{debug, warn};
use netlink::Netlink;
use netlink::NlPayload;
use std::{mem, slice};
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
        self.send(TASKSTATS_CMD_ATTR_PID as u16, tid.as_buf())?;

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
        self.send(TASKSTATS_CMD_ATTR_TGID as u16, tgid.as_buf())?;

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

    /// Register listener with the specific cpumask
    ///
    /// # Arguments
    /// * `cpu_mask` - cpumask is specified as an ascii string of comma-separated cpu ranges e.g.
    ///   to listen to exit data from cpus 1,2,3,5,7,8 the cpumask would be "1-3,5,7-8".
    pub fn register_cpumask(&self, cpu_mask: &str) -> Result<()> {
        self.send(
            TASKSTATS_CMD_ATTR_REGISTER_CPUMASK as u16,
            cpu_mask.as_bytes(),
        )?;
        Ok(())
    }

    /// Deregister listener with the specific cpumask
    /// If userspace forgets to deregister interest in cpus before closing the listening socket,
    /// the kernel cleans up its interest set over time. However, for the sake of efficiency,
    /// an explicit deregistration is advisable.
    ///
    /// # Arguments
    /// * `cpu_mask` - cpumask is specified as an ascii string of comma-separated cpu ranges e.g.
    ///   to listen to exit data from cpus 1,2,3,5,7,8 the cpumask would be "1-3,5,7-8".
    pub fn deregister_cpumask(&self, cpu_mask: &str) -> Result<()> {
        self.send(
            TASKSTATS_CMD_ATTR_DEREGISTER_CPUMASK as u16,
            cpu_mask.as_bytes(),
        )?;
        Ok(())
    }

    /// Listen registered cpumask's.
    /// If no messages are available at the socket, the receive call
    /// wait for a message to arrive, unless the socket is nonblocking.
    ///
    /// # Return
    /// * `Ok(Vec<TaskStats>)`: vector with stats messages. If the current task is NOT the last
    ///   one in its thread group, only one message is returned in the vector.
    ///   However, if it is the last task, an additional element containing the per-thread
    ///   group ID (tgid) statistics is also included. This additional element sums up
    ///   the statistics for all threads within the thread group, both past and present
    pub fn listen_registered(&self) -> Result<Vec<TaskStats>> {
        let resp = self.netlink.recv_response()?;
        let mut stats_vec = Vec::new();

        for na in resp.payload_as_nlattrs() {
            match na.header.nla_type as u32 {
                TASKSTATS_TYPE_NULL => break,
                TASKSTATS_TYPE_AGGR_PID | TASKSTATS_TYPE_AGGR_TGID => {
                    for inner in na.payload_as_nlattrs() {
                        match inner.header.nla_type as u32 {
                            TASKSTATS_TYPE_PID => debug!("Received TASKSTATS_TYPE_PID"),
                            TASKSTATS_TYPE_TGID => debug!("Received TASKSTATS_TYPE_TGID"),
                            TASKSTATS_TYPE_STATS => {
                                stats_vec.push(TaskStats::from(inner.payload()));
                            }
                            unknown => println!("Skipping unknown nla_type: {}", unknown),
                        }
                    }
                }
                unknown => println!("Skipping unknown nla_type: {}", unknown),
            }
        }
        if !stats_vec.is_empty() {
            return Ok(stats_vec);
        }
        Err(Error::Unknown(
            "no TASKSTATS_TYPE_STATS found in response".to_string(),
        ))
    }

    /// Set receiver buffer size in bytes (SO_RCVBUF socket option, see socket(7))
    ///
    /// # Arguments
    /// * `payload` - buffer size in bytes. The kernel doubles this value
    ///   (to allow space for bookkeeping overhead). The default value is set by the
    ///   /proc/sys/net/core/rmem_default file, and the maximum allowed value is set by the
    ///   /proc/sys/net/core/rmem_max file. The minimum (doubled) value for this option is 256.
    pub fn set_rx_buf_sz<T>(&self, payload: T) -> Result<()> {
        self.netlink
            .set_rx_buf_sz(payload)
            .map_err(|err| err.into())
    }

    /// Get receiver buffer size in bytes (SO_RCVBUF socket option, see socket(7))
    ///
    /// # Return
    /// * `usize` buffer size in bytes.
    ///   Kernel returns doubled value, that have been set using [set_rx_buf_sz]
    pub fn get_rx_buf_sz(&self) -> Result<usize> {
        self.netlink.get_rx_buf_sz().map_err(|err| err.into())
    }

    pub fn send(&self, taskstats_cmd: u16, data: &[u8]) -> Result<()> {
        self.netlink.send_cmd(
            self.ts_family_id,
            TASKSTATS_CMD_GET as u8,
            taskstats_cmd,
            data,
        )?;
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(test_priv)]
    #[test]
    fn test_pid_stats() {
        let client = Client::open().unwrap();
        let ts = client.pid_stats(std::process::id()).unwrap();

        // Just asserts some fields which do likely have positive values
        assert!(ts.delays.cpu.delay_total.as_nanos() > 0);
        assert!(ts.cpu.virtual_time_total.as_nanos() > 0);
    }

    #[cfg(test_priv)]
    #[test]
    fn test_tgid_stats() {
        let client = Client::open().unwrap();
        let ts = client.tgid_stats(std::process::id()).unwrap();

        // Just asserts some fields which do likely have positive values
        assert!(ts.delays.cpu.delay_total.as_nanos() > 0);
        assert!(ts.cpu.virtual_time_total.as_nanos() > 0);
    }
}
