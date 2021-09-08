use futures::future::TryFutureExt;
use rpc::notifications::notifications_server::NotificationsServer;
use services::notifications::NotificationsService;
use tonic::transport::Server;

mod services;
mod vmcompute;
mod vmsocket;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let notifications = NotificationsService::default();

    let incoming = {
        let listener = vmsocket::bind_hyperv_socket(
            vmcompute::get_wsl_vmid()?.expect("WSL VM not found!"),
            7070,
        )?;

        async_stream::stream! {
            while let item = listener.accept().map_ok(|(st, _)| st).await {
                yield item;
            }
        }
    };

    Server::builder()
        .add_service(NotificationsServer::new(notifications))
        .serve_with_incoming(incoming)
        .await?;

    Ok(())
}
