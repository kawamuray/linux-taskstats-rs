use crate::AsBuf;
use libc;
use log::debug;
use netlink_sys::{self as nl, Socket, SocketAddr};
use std::io;
use std::mem;
use std::process;
use std::slice;
use thiserror::Error;

const MAX_MESSAGE_SIZE: usize = 1024;

#[derive(Debug, Error)]
pub enum Error {
    #[error("error in I/O with netlink socket: {0}")]
    SocketIo(#[from] io::Error),
    #[error("corrupted data read from netlink socket: {0}")]
    Protocol(String),
    #[error("error response received from remote")]
    ErrorResponse,
}

pub type Result<T> = std::result::Result<T, Error>;

mod nlmsg {
    use crate::c_headers::NLMSG_ALIGNTO;
    use std::mem;

    pub const HDRLEN: usize = align(mem::size_of::<libc::nlmsghdr>());
    pub const GENL_HDRLEN: usize = align(mem::size_of::<libc::genlmsghdr>());

    pub const fn align(len: usize) -> usize {
        (len + NLMSG_ALIGNTO as usize - 1) & !(NLMSG_ALIGNTO as usize - 1)
    }

    #[inline]
    pub fn is_valid(nlh: &libc::nlmsghdr, len: usize) -> bool {
        len >= mem::size_of::<libc::nlmsghdr>()
            && nlh.nlmsg_len as usize >= mem::size_of::<libc::nlmsghdr>()
            && nlh.nlmsg_len as usize <= len
    }
}

mod nla {
    use std::mem;

    pub const HDRLEN: usize = align(mem::size_of::<libc::nlattr>());

    pub const fn align(len: usize) -> usize {
        (len + libc::NLA_ALIGNTO as usize - 1) & !(libc::NLA_ALIGNTO as usize - 1)
    }

    #[inline]
    pub fn payload(na: &libc::nlattr) -> *const u8 {
        unsafe { (na as *const libc::nlattr as *const u8).offset(HDRLEN as isize) }
    }

    #[inline]
    pub fn next(na: &libc::nlattr) -> &libc::nlattr {
        unsafe {
            &*((na as *const libc::nlattr as *const u8).offset(align(na.nla_len as usize) as isize)
                as *const libc::nlattr)
        }
    }
}

/// Trait abstracting netlink socket IO.
/// This trait is only meant to replace socket implementation at unit testing.
pub trait NlSocket {
    type Addr;

    fn send_to(&self, buf: &[u8], addr: &Self::Addr) -> io::Result<usize>;

    fn recv(&self, buf: &mut [u8]) -> io::Result<usize>;
}

impl NlSocket for nl::Socket {
    type Addr = nl::SocketAddr;

    fn send_to(&self, buf: &[u8], addr: &Self::Addr) -> io::Result<usize> {
        self.send_to(buf, addr, 0)
    }

    fn recv(&self, mut buf: &mut [u8]) -> io::Result<usize> {
        self.recv(&mut buf, 0)
    }
}

/// Netlink protocol implementation specifically for taskstats querying.
pub struct Netlink<S: NlSocket = nl::Socket> {
    sock: S,
    remote_addr: S::Addr,
    mypid: u32,
}

impl Netlink<nl::Socket> {
    pub fn open() -> Result<Netlink<nl::Socket>> {
        let mut sock = Socket::new(nl::protocols::NETLINK_GENERIC)?;
        let addr = SocketAddr::new(0, 0);
        sock.bind(&addr)?;
        Ok(Netlink {
            sock,
            remote_addr: SocketAddr::new(0, 0),
            mypid: process::id(),
        })
    }
}

impl<S: NlSocket> Netlink<S> {
    pub fn send_cmd(
        &self,
        nlmsg_type: u16,
        genl_cmd: u8,
        nla_type: u16,
        nla_data: &[u8],
    ) -> Result<()> {
        debug!(
            "Sending nl cmd: type={}, genl_cmd={}, nla_type={} nla_data.len={}",
            nlmsg_type,
            genl_cmd,
            nla_type,
            nla_data.len()
        );

        let attr = libc::nlattr {
            nla_type,
            nla_len: nla::align(nla::HDRLEN + nla_data.len()) as u16,
        };
        let mut buf = [0u8; MAX_MESSAGE_SIZE];
        let bufp = buf.as_mut_ptr();
        unsafe {
            std::ptr::copy_nonoverlapping(
                &attr as *const libc::nlattr as *const u8,
                bufp,
                mem::size_of::<libc::nlattr>(),
            );
            std::ptr::copy_nonoverlapping(
                nla_data.as_ptr() as *const u8,
                bufp.offset(nla::HDRLEN as isize),
                nla_data.len(),
            );
        }

        let nlmsg_len = nlmsg::HDRLEN + nlmsg::GENL_HDRLEN + attr.nla_len as usize;
        let msg = GenNlMsg {
            nlmsg_header: libc::nlmsghdr {
                nlmsg_len: nlmsg_len as u32,
                nlmsg_type,
                nlmsg_flags: libc::NLM_F_REQUEST as u16,
                nlmsg_seq: 0,
                nlmsg_pid: self.mypid,
            },
            genlmsg_header: libc::genlmsghdr {
                cmd: genl_cmd,
                version: 0x1,
                reserved: 0x0,
            },
            buf,
        };
        debug!("Sending msg of size={}", nlmsg_len);

        let mut send_buf = &msg.as_buf()[..msg.nlmsg_header.nlmsg_len as usize];
        loop {
            let sent_size = self.sock.send_to(&send_buf, &self.remote_addr)?;
            if sent_size == send_buf.len() {
                break;
            }
            send_buf = &send_buf[sent_size..];
        }
        Ok(())
    }

    pub fn recv_response(&self) -> Result<GenNlMsg> {
        let mut msg: GenNlMsg = unsafe { mem::zeroed() };
        let rep_len = self.sock.recv(msg.as_buf_mut())?;

        debug!(
            "Received msg: size={}, type={}, nlmsg_len={}",
            rep_len, msg.nlmsg_header.nlmsg_type, msg.nlmsg_header.nlmsg_len
        );

        if !nlmsg::is_valid(&msg.nlmsg_header, rep_len) {
            return Err(Error::Protocol(format!(
                "header len: {}, recv size: {}",
                msg.nlmsg_header.nlmsg_len, rep_len
            )));
        }
        if msg.nlmsg_header.nlmsg_len as usize > mem::size_of::<GenNlMsg>() {
            return Err(Error::Protocol(format!(
                "too large message size: {}",
                msg.nlmsg_header.nlmsg_len
            )));
        }

        if msg.nlmsg_header.nlmsg_type == libc::NLMSG_ERROR as u16 {
            return Err(Error::ErrorResponse);
        }

        Ok(msg)
    }
}

pub trait NlPayload {
    fn payload(&self) -> &[u8];

    #[inline]
    fn payload_len(&self) -> usize {
        self.payload().len()
    }

    fn payload_as<T>(&self) -> &T {
        if mem::size_of::<T>() > self.payload_len() {
            panic!(
                "attempt to cast buffer into type that has larger size than buf length: {} > {}",
                mem::size_of::<T>(),
                self.payload_len()
            );
        }
        unsafe { &*(self.payload().as_ptr() as *const T) }
    }

    fn payload_as_nlattrs(&self) -> NlAttrs<'_> {
        NlAttrs {
            next: Some(self.payload_as::<libc::nlattr>()),
            rem_size: self.payload_len(),
        }
    }
}

#[repr(C)]
pub struct GenNlMsg {
    pub nlmsg_header: libc::nlmsghdr,
    pub genlmsg_header: libc::genlmsghdr,
    pub buf: [u8; MAX_MESSAGE_SIZE],
}

impl NlPayload for GenNlMsg {
    fn payload(&self) -> &[u8] {
        let len = self.nlmsg_header.nlmsg_len as usize - nlmsg::HDRLEN - nlmsg::GENL_HDRLEN;
        &self.buf[..len]
    }
}

pub struct NlAttr<'a> {
    pub header: &'a libc::nlattr,
}

impl<'a> NlPayload for NlAttr<'a> {
    fn payload(&self) -> &[u8] {
        let len = self.header.nla_len as usize - nla::HDRLEN;
        unsafe { slice::from_raw_parts(nla::payload(self.header), len) }
    }
}

pub struct NlAttrs<'a> {
    next: Option<&'a libc::nlattr>,
    rem_size: usize,
}

impl<'a> Iterator for NlAttrs<'a> {
    type Item = NlAttr<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ret) = self.next.take() {
            self.rem_size -= nla::align(ret.nla_len as usize);
            if self.rem_size >= nla::HDRLEN {
                let next = nla::next(&ret);
                self.next.replace(next);
            }
            return Some(NlAttr { header: ret });
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{SocketAddr, UdpSocket};
    use std::ptr;

    const NLMSG_TYPE: u16 = 32;
    const GENL_CMD: u8 = 3;
    const NLA_TYPE: u16 = 17;
    const PID: u32 = 1234;
    const PAYLOAD: &'static str = "Hello";

    impl NlSocket for UdpSocket {
        type Addr = SocketAddr;

        fn send_to(&self, buf: &[u8], addr: &Self::Addr) -> io::Result<usize> {
            self.send_to(buf, addr)
        }

        fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
            self.recv(buf)
        }
    }

    fn nl_sock() -> UdpSocket {
        UdpSocket::bind("localhost:0").unwrap()
    }

    fn nl(serv_sock: &UdpSocket) -> Netlink<UdpSocket> {
        let sock = nl_sock();
        Netlink {
            sock,
            remote_addr: serv_sock.local_addr().unwrap(),
            mypid: PID,
        }
    }

    #[test]
    fn test_send_cmd() {
        let serv_sock = nl_sock();
        let nl = nl(&serv_sock);

        nl.send_cmd(NLMSG_TYPE, GENL_CMD, NLA_TYPE, PAYLOAD.as_bytes())
            .unwrap();
        let mut buf = [0u8; 256];
        let size = serv_sock.recv(&mut buf).unwrap();

        let expect_size =
            nlmsg::HDRLEN + nlmsg::GENL_HDRLEN + nla::HDRLEN + nla::align(PAYLOAD.as_bytes().len());
        assert_eq!(expect_size, size);

        let n = unsafe { &*(&buf as *const u8 as *const libc::nlmsghdr) };
        assert_eq!(expect_size, n.nlmsg_len as usize);
        assert_eq!(NLMSG_TYPE, n.nlmsg_type);
        assert_eq!(PID, n.nlmsg_pid);

        let g = unsafe {
            &*((&buf as *const u8).offset(nlmsg::HDRLEN as isize) as *const libc::genlmsghdr)
        };
        assert_eq!(GENL_CMD, g.cmd);

        let payload = unsafe {
            slice::from_raw_parts(
                (&buf as *const u8)
                    .offset((nlmsg::HDRLEN + nlmsg::GENL_HDRLEN + nla::HDRLEN) as isize),
                PAYLOAD.len(),
            )
        };
        assert_eq!(PAYLOAD.as_bytes(), payload);
    }

    #[test]
    fn test_recv_response() {
        let serv_sock = nl_sock();
        let nl = nl(&serv_sock);

        let mut pos = 0;

        let mut buf = [0u8; 256];
        let nlmsg_len = nlmsg::HDRLEN + nlmsg::GENL_HDRLEN + PAYLOAD.len();
        let addr = nl.sock.local_addr().unwrap();
        let n = libc::nlmsghdr {
            nlmsg_len: nlmsg_len as u32,
            nlmsg_type: NLMSG_TYPE,
            nlmsg_flags: 0,
            nlmsg_seq: 0,
            nlmsg_pid: PID,
        };
        unsafe {
            ptr::copy_nonoverlapping(
                &n as *const libc::nlmsghdr as *const u8,
                buf.as_mut_ptr().offset(pos as isize),
                mem::size_of::<libc::nlmsghdr>(),
            );
        }
        pos += nlmsg::HDRLEN;

        let g = libc::genlmsghdr {
            cmd: GENL_CMD,
            version: 0x1,
            reserved: 0x0,
        };
        unsafe {
            ptr::copy_nonoverlapping(
                &g as *const libc::genlmsghdr as *const u8,
                buf.as_mut_ptr().offset(pos as isize),
                mem::size_of::<libc::genlmsghdr>(),
            );
        }
        pos += nlmsg::GENL_HDRLEN;

        unsafe {
            ptr::copy_nonoverlapping(
                PAYLOAD.as_ptr(),
                buf.as_mut_ptr().offset(pos as isize),
                PAYLOAD.len(),
            );
        }
        pos += PAYLOAD.len();

        serv_sock.send_to(&buf[..pos], &addr).unwrap();

        let resp = nl.recv_response().unwrap();
        assert_eq!(n.nlmsg_len, resp.nlmsg_header.nlmsg_len);
        assert_eq!(n.nlmsg_type, resp.nlmsg_header.nlmsg_type);
        assert_eq!(n.nlmsg_pid, resp.nlmsg_header.nlmsg_pid);
        assert_eq!(g.cmd, resp.genlmsg_header.cmd);
        assert_eq!(PAYLOAD.as_bytes(), &resp.buf[..PAYLOAD.len()]);
    }

    #[test]
    fn test_nlpayload() {
        struct Msg<'a>(&'a [u8]);
        impl<'a> NlPayload for Msg<'a> {
            fn payload(&self) -> &[u8] {
                self.0
            }
        }

        let n: u32 = 1234;
        let m = Msg(unsafe {
            slice::from_raw_parts(&n as *const u32 as *const u8, mem::size_of::<u32>())
        });
        assert_eq!(mem::size_of::<u32>(), m.payload_len());
        assert_eq!(n, *m.payload_as());
    }

    #[test]
    fn test_nlpayload_nlattrs() {
        let mut buf = [0u8; 256];

        fn add_na<T>(buf: &mut [u8], pos: &mut usize, val: T) {
            let header =
                unsafe { &mut *(buf.as_mut_ptr().offset(*pos as isize) as *mut libc::nlattr) };
            header.nla_type = 0;
            header.nla_len = nla::align(nla::HDRLEN + mem::size_of::<T>()) as u16;
            unsafe {
                ptr::copy_nonoverlapping(
                    &val as *const T as *const u8,
                    buf.as_mut_ptr().offset((*pos + nla::HDRLEN) as isize),
                    mem::size_of::<T>(),
                )
            };
            *pos += header.nla_len as usize;
        }

        let header = unsafe { &mut *(buf.as_mut_ptr() as *mut libc::nlattr) };
        header.nla_type = 0;
        header.nla_len =
            nla::align(nla::HDRLEN + nla::align(nla::HDRLEN + mem::size_of::<char>()) * 3) as u16;

        let mut pos = nla::HDRLEN;
        add_na(&mut buf, &mut pos, 'a');
        add_na(&mut buf, &mut pos, 'b');
        add_na(&mut buf, &mut pos, 'c');

        let outer = NlAttr {
            header: unsafe { &*(buf.as_ptr() as *const libc::nlattr) },
        };
        let mut iter = outer.payload_as_nlattrs();
        assert_eq!(Some('a' as u8), iter.next().map(|x| *x.payload_as()));
        assert_eq!(Some('b' as u8), iter.next().map(|x| *x.payload_as()));
        assert_eq!(Some('c' as u8), iter.next().map(|x| *x.payload_as()));
        assert_eq!(None, iter.next().map(|x| *x.payload_as::<u8>()));
    }

    #[test]
    fn test_gennlmsg_payload() {
        const LEN: usize = 3;
        let mut msg: GenNlMsg = unsafe { mem::zeroed() };
        msg.nlmsg_header.nlmsg_len = nlmsg::align(nlmsg::HDRLEN + nlmsg::GENL_HDRLEN + LEN) as u32;
        let p = msg.payload();
        assert_eq!(msg.buf.as_ptr(), p.as_ptr());
        assert_eq!(nlmsg::align(LEN), p.len());
    }

    #[test]
    fn test_nlattr_payload() {
        const LEN: usize = 3;
        let na = libc::nlattr {
            nla_len: nla::align(nla::HDRLEN + LEN) as u16,
            nla_type: 0,
        };
        let nlattr = NlAttr { header: &na };
        let p = nlattr.payload();
        let expect_p =
            unsafe { (&na as *const libc::nlattr as *const u8).offset(nla::HDRLEN as isize) };
        assert_eq!(expect_p, p.as_ptr());
        assert_eq!(nlmsg::align(LEN), p.len());
    }
}
