use scopeguard::ScopeGuard;
use std::{io, net, os::windows::prelude::FromRawSocket, sync::Once};
use tokio::net::TcpStream;
use uuid::Uuid;
use windows::core::GUID;
use windows::Win32::Networking::WinSock::{
    accept, bind, closesocket, connect, listen, socket, WSAGetLastError, AF_HYPERV, INVALID_SOCKET,
    SOCKADDR, SOCKET, SOCK_STREAM, SOMAXCONN,
};

struct HyperVSocketAddr {
    pub family: u32,
    pub _reserved: u16,
    pub vm_id: windows::core::GUID,
    pub service_id: windows::core::GUID,
}

/// Initialise the network stack for Windows.
fn init() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        // Let standard library call `WSAStartup` for us, we can't do it
        // ourselves because otherwise using any type in `std::net` would panic
        // when it tries to call `WSAStartup` a second time.
        drop(std::net::UdpSocket::bind("127.0.0.1:0"));
    });
}

fn winsock_error() -> std::io::Error {
    std::io::Error::from_raw_os_error(unsafe { WSAGetLastError() })
}

pub struct HyperVSocket(SOCKET);

impl Drop for HyperVSocket {
    fn drop(&mut self) {
        unsafe { closesocket(self.0) };
    }
}

impl HyperVSocket {
    #[allow(dead_code)]
    pub fn bind(vmid: Uuid, port: u32) -> io::Result<HyperVSocket> {
        init();
        let local_addr = get_addr(vmid, port);

        // HV_PROTOCOL_RAW is defined as 1
        let fd = unsafe { socket(AF_HYPERV as _, SOCK_STREAM as _, 1) };
        if fd == INVALID_SOCKET {
            return Err(winsock_error());
        }
        let _guard = scopeguard::guard((), |_| {
            unsafe { closesocket(fd) };
        });

        let result = unsafe {
            bind(
                fd,
                &local_addr as *const _ as *const SOCKADDR,
                std::mem::size_of::<HyperVSocketAddr>() as _,
            )
        };
        if result < 0 {
            return Err(winsock_error());
        }

        let result = unsafe { listen(fd, SOMAXCONN as _) };
        if result < 0 {
            return Err(winsock_error());
        }

        // defuse guard
        ScopeGuard::into_inner(_guard);

        Ok(HyperVSocket(fd))
    }

    #[allow(dead_code)]
    pub fn accept(&self) -> std::io::Result<TcpStream> {
        let fd = unsafe { accept(self.0, std::ptr::null_mut(), std::ptr::null_mut()) };
        if fd == INVALID_SOCKET {
            return Err(winsock_error());
        }

        unsafe { into_tcp_stream(fd) }
    }

    pub fn connect(vmid: Uuid, port: u32) -> io::Result<TcpStream> {
        init();
        let local_addr = get_addr(vmid, port);

        let fd = unsafe { socket(AF_HYPERV as _, SOCK_STREAM as _, 1) };
        if fd == INVALID_SOCKET {
            return Err(winsock_error());
        }
        let _guard = scopeguard::guard((), |_| {
            unsafe { closesocket(fd) };
        });

        let result = unsafe {
            connect(
                fd,
                &local_addr as *const _ as *const SOCKADDR,
                std::mem::size_of::<HyperVSocketAddr>() as _,
            )
        };

        if result < 0 {
            return Err(winsock_error());
        }

        ScopeGuard::into_inner(_guard);

        unsafe { into_tcp_stream(fd) }
    }
}

fn get_addr(vmid: Uuid, port: u32) -> HyperVSocketAddr {
    let mut local_addr: HyperVSocketAddr = unsafe { std::mem::zeroed() };
    local_addr.family = AF_HYPERV as _;
    let service_id: Uuid = "00000000-facb-11e6-bd58-64006a7986d3".parse().unwrap();
    let fields = service_id.as_fields();
    local_addr.service_id = GUID::from_values(port, fields.1, fields.2, *fields.3);
    let fields = vmid.as_fields();
    local_addr.vm_id = GUID::from_values(fields.0, fields.1, fields.2, *fields.3);
    local_addr
}

unsafe fn into_tcp_stream(socket: SOCKET) -> io::Result<TcpStream> {
    let stream = net::TcpStream::from_raw_socket(socket.0 as _);
    stream.set_nonblocking(true)?;
    TcpStream::from_std(stream)
}
