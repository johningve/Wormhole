use std::{error::Error, path::Path};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixStream,
};

use vmsocket::VmSocket;
use zbus::{Address, ConnectionBuilder};

mod vmsocket;

const CONNECT: &[u8] = b"connect\r\n";

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

    // connect to the bus socket and pipe it to vm socket

    let vm_socket = VmSocket::bind(7070)?;

    let Address::Unix(addr) = Address::session()?;

    let distro_name = std::env::var("WSL_DISTRO_NAME")? + "\r\n";
    let user_name = whoami::username() + "\r\n";
    let home_folder = std::env::var("HOME")? + "\r\n";

    let header = [
        ((distro_name.len() + user_name.len() + home_folder.len()) as u32)
            .to_le_bytes()
            .as_ref(),
        distro_name.as_bytes(),
        user_name.as_bytes(),
        home_folder.as_bytes(),
    ]
    .concat();

    loop {
        let mut buffer = [0; 9];

        let mut vm_stream = vm_socket.accept()?;
        let mut dbus_stream = UnixStream::connect(Path::new(&addr)).await?;

        vm_stream.write_all(&header).await?;

        // FIXME: DOS
        vm_stream.read_exact(&mut buffer).await?;
        if buffer != CONNECT {
            log::warn!("unknown reply");
            continue;
        }

        tokio::spawn(async move {
            tokio::io::copy_bidirectional(&mut dbus_stream, &mut vm_stream).await
        });
    }
}
