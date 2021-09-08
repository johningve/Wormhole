use std::{net, os::windows::prelude::FromRawSocket, sync::Once};

use bindings::Windows::Win32::Networking::WinSock::{
    bind, closesocket, listen, socket, WSAGetLastError, AF_HYPERV, INVALID_SOCKET, SOCKADDR,
    SOCK_STREAM, SOMAXCONN,
};
use scopeguard::ScopeGuard;
use tokio::net::TcpListener;
use uuid::Uuid;
use windows::Guid;

struct HyperVSocketAddr {
    pub family: u16,
    pub reserved: u16,
    pub vm_id: windows::Guid,
    pub service_id: windows::Guid,
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
    std::io::Error::from_raw_os_error(unsafe { WSAGetLastError() }.0)
}

pub fn bind_hyperv_socket(vmid: Uuid, port: u32) -> std::io::Result<TcpListener> {
    init();
    let mut local_addr: HyperVSocketAddr = unsafe { std::mem::zeroed() };
    let service_id: Uuid = "00000000-facb-11e6-bd58-64006a7986d3".parse().unwrap();
    let fields = service_id.as_fields();
    local_addr.service_id = Guid::from_values(port, fields.1, fields.2, *fields.3);
    let fields = vmid.as_fields();
    local_addr.vm_id = Guid::from_values(fields.0, fields.1, fields.2, *fields.3);

    // HV_PROTOCOL_RAW is defined as 1
    let fd = unsafe { socket(AF_HYPERV as _, SOCK_STREAM as _, 1) };
    if fd.0 == INVALID_SOCKET as usize {
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

    let listener = unsafe { net::TcpListener::from_raw_socket(fd.0 as _) };
    listener.set_nonblocking(true)?;
    TcpListener::from_std(listener)
}
