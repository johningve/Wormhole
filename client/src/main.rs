use futures::future;
use rpc::notifications::{notifications_client::NotificationsClient, NotifyRequest};
use std::{convert::TryFrom, error::Error};
use tonic::transport::{Endpoint, Uri};
use tower::service_fn;
use vmsocket::VmSocket;

mod vmsocket;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let channel = Endpoint::try_from("http://[::]:0")?
        .connect_with_connector(service_fn(|_: Uri| future::ready(VmSocket::connect(7070))))
        .await?;

    let mut client = NotificationsClient::new(channel);

    let request = tonic::Request::new(NotifyRequest {
        name: "John Ingve".into(),
    });

    let response = client.notify(request).await?;

    println!("Response: {:?}", response);

    Ok(())
}
