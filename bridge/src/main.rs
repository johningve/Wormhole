use serde::Serialize;
use std::{error::Error, path::Path};
use tokio::{io::AsyncWriteExt, net::UnixStream};
use zvariant_derive::Type;

use vmsocket::VmSocket;
use zbus::{Address, ConnectionBuilder};

mod services;
mod vmsocket;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    // set up a regular D-Bus connection and initialize services.
    let dbus_connection = ConnectionBuilder::session()?
        .internal_executor(false)
        .build()
        .await?;

    // start up a task for the zbus executor to use.
    /* let handle =  */
    {
        // need to clone connection before moving into async task.
        let dbus_connection = dbus_connection.clone();
        tokio::spawn(async move {
            loop {
                dbus_connection.executor().tick().await;
            }
        })
    };

    dbus_connection
        .request_name("org.freedesktop.impl.portal.desktop.wsl")
        .await?;

    // handle.await?;

    services::init_all(&dbus_connection).await?;

    // connect to the bus socket and pipe it to vm socket

    let vm_socket = VmSocket::bind(7070)?;

    let Address::Unix(addr) = Address::session()?;

    let info = DistroInfo::new(
        std::env::var("WSL_DISTRO_NAME")?,
        whoami::username(),
        std::env::var("HOME")?,
    );

    let ctxt = zvariant::EncodingContext::<byteorder::NativeEndian>::new_dbus(0);
    let info_bytes = zvariant::to_bytes(ctxt, &info)?;

    let header = [
        (info_bytes.len() as u32).to_le_bytes().as_ref(),
        &info_bytes,
    ]
    .concat();

    loop {
        let mut vm_stream = vm_socket.accept()?;
        let mut dbus_stream = UnixStream::connect(Path::new(&addr)).await?;

        vm_stream.write_all(&header).await?;

        tokio::spawn(async move {
            tokio::io::copy_bidirectional(&mut dbus_stream, &mut vm_stream).await
        });
    }
}

#[derive(Serialize, Type)]
struct DistroInfo {
    distro_name: String,
    user_name: String,
    home_folder: String,
}

impl DistroInfo {
    pub fn new(distro_name: String, user_name: String, home_folder: String) -> Self {
        Self {
            distro_name,
            home_folder,
            user_name,
        }
    }
}
