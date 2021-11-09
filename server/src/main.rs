use std::{thread, time::Duration};

use bindings::Windows::Win32::{
    System::Com::{CoInitializeEx, COINIT_MULTITHREADED},
    UI::HiDpi::{SetProcessDpiAwareness, PROCESS_PER_MONITOR_DPI_AWARE},
};
use rpc::{
    filechooser::file_chooser_server::FileChooserServer,
    notifications::notifications_server::NotificationsServer,
};
use services::notifications::NotificationsService;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
use tonic::transport::Server;

use crate::services::filechooser::FileChooserService;

use zbus::Socket;

mod services;
mod toasthelper;
mod util;
mod vmcompute;
mod vmsocket;
mod wslpath;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    unsafe { SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE) }.unwrap();
    unsafe { CoInitializeEx(std::ptr::null_mut(), COINIT_MULTITHREADED) }.unwrap();

    // let incoming = async_stream::stream! {
    //     let listener = vmsocket::HyperVSocket::bind(
    //         vmcompute::get_wsl_vmid()?,
    //         7070,
    //     )?;

    //     loop {
    //         yield listener.accept();
    //     }
    // };

    // Server::builder()
    //     .add_service(NotificationsServer::new(NotificationsService::default()))
    //     .add_service(FileChooserServer::new(FileChooserService::default()))
    //     .serve_with_incoming(incoming)
    //     .await?;

    let mut stream = vmsocket::HyperVSocket::connect(vmcompute::get_wsl_vmid()?, 7070)?;

    // read the header size
    let mut size_buffer: [u8; 4] = [0; 4];
    stream.read_exact(&mut size_buffer).await?;

    // read the header
    let mut header_buffer = vec![0; u32::from_le_bytes(size_buffer) as _];
    stream.read_exact(&mut header_buffer).await?;

    // read the individual header fields
    let mut distro_name = String::new();
    let mut user_name = String::new();
    let mut home_path = String::new();

    let mut offset;
    offset = (&header_buffer[..]).read_line(&mut distro_name).await?;
    offset += (&header_buffer[offset..]).read_line(&mut user_name).await?;
    (&header_buffer[offset..]).read_line(&mut home_path).await?;

    // TODO: might want to make this a constant in a shared package
    stream.write_all(b"connect\r\n").await?;

    std::env::set_var(
        "ZBUS_WSL_HOME",
        wslpath::to_windows(distro_name.trim_end(), home_path.trim_end()),
    );
    std::env::set_var("ZBUS_WSL_USER", user_name.trim_end());

    let mut connection = zbus::ConnectionBuilder::socket(stream)
        .name("org.freedesktop.impl.portal.desktop.windows")?
        .build()
        .await?;

    println!("done?");

    loop {
        thread::sleep(Duration::from_millis(1000));
    }

    Ok(())
}
