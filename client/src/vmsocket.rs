use nix::{
    sys::socket::{connect, socket, AddressFamily, SockAddr, SockFlag, SockType},
    unistd::close,
};
use std::{io, net, os::unix::prelude::FromRawFd};
use tokio::net::TcpStream;

pub struct VmSocket;

impl VmSocket {
    pub fn connect(port: u32) -> io::Result<TcpStream> {
        let addr = SockAddr::new_vsock(libc::VMADDR_CID_HOST, port);

        let fd = socket(
            AddressFamily::Vsock,
            SockType::Stream,
            SockFlag::empty(),
            None,
        )?;

        // TODO: figure out if we should bind or connect?
        if let Err(e) = connect(fd, &addr) {
            let _ = close(fd);
            return Err(e.into());
        }

        let stream = unsafe { net::TcpStream::from_raw_fd(fd) };
        stream.set_nonblocking(true)?;
        TcpStream::from_std(stream)
    }
}
