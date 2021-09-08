use std::{
    io,
    os::unix::{net::UnixStream, prelude::FromRawFd},
};

use nix::{
    sys::socket::{connect, socket, AddressFamily, SockAddr, SockFlag, SockType},
    unistd::close,
};

pub struct VmSocket;

impl VmSocket {
    pub fn connect(port: u32) -> io::Result<UnixStream> {
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

        Ok(unsafe { UnixStream::from_raw_fd(fd) })
    }
}
