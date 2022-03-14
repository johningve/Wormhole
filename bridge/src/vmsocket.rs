use nix::{
    sys::socket::{
        accept, bind, connect, listen, socket, AddressFamily, SockAddr, SockFlag, SockType,
    },
    unistd::close,
};
use std::{
    io, net,
    os::unix::prelude::{FromRawFd, RawFd},
};
use tokio::net::TcpStream;

pub struct VmSocket(RawFd);

impl VmSocket {
    #[allow(dead_code)]
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

        unsafe { into_tcp_stream(fd) }
    }

    pub fn bind(port: u32) -> io::Result<VmSocket> {
        let addr = SockAddr::new_vsock(libc::VMADDR_CID_ANY, port);

        let fd = socket(
            AddressFamily::Vsock,
            SockType::Stream,
            SockFlag::empty(),
            None,
        )?;

        if let Err(e) = bind(fd, &addr) {
            let _ = close(fd);
            return Err(e.into());
        }

        if let Err(e) = listen(fd, 128) {
            let _ = close(fd);
            return Err(e.into());
        }

        Ok(VmSocket(fd))
    }

    pub fn accept(&self) -> io::Result<TcpStream> {
        let fd = accept(self.0)?;
        unsafe { into_tcp_stream(fd) }
    }
}

impl Drop for VmSocket {
    fn drop(&mut self) {
        let _ = close(self.0);
    }
}

unsafe fn into_tcp_stream(fd: RawFd) -> io::Result<TcpStream> {
    let stream = net::TcpStream::from_raw_fd(fd);
    stream.set_nonblocking(true)?;
    TcpStream::from_std(stream)
}
