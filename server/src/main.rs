use rpc::notifications::notifications_server::NotificationsServer;
use services::notifications::NotificationsService;
use tonic::transport::Server;

mod services;
mod toasthelper;
mod util;
mod vmcompute;
mod vmsocket;
mod wslpath;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let notifications = NotificationsService::default();

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
        .add_service(NotificationsServer::new(notifications))
        .serve_with_incoming(incoming)
        .await?;

    Ok(())
}
