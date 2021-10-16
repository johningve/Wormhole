use bindings::Windows::Win32::{
    System::Com::{CoInitializeEx, COINIT_MULTITHREADED},
    UI::HiDpi::{SetProcessDpiAwareness, PROCESS_PER_MONITOR_DPI_AWARE},
};
use rpc::{
    filechooser::file_chooser_server::FileChooserServer,
    notifications::notifications_server::NotificationsServer,
};
use services::notifications::NotificationsService;
use tonic::transport::Server;

use crate::services::filechooser::FileChooserService;

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

    let incoming = async_stream::stream! {
        let listener = vmsocket::HyperVSocket::bind(
            vmcompute::get_wsl_vmid()?,
            7070,
        )?;

        loop {
            yield listener.accept();
        }
    };

    Server::builder()
        .add_service(NotificationsServer::new(NotificationsService::default()))
        .add_service(FileChooserServer::new(FileChooserService::default()))
        .serve_with_incoming(incoming)
        .await?;

    Ok(())
}
