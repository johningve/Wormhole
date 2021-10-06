use futures::future;
use std::{convert::TryFrom, error::Error};
use tonic::transport::{Endpoint, Uri};
use tower::service_fn;
use vmsocket::VmSocket;
use zbus::ConnectionBuilder;

mod services;
mod vmsocket;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let grpc_channel = Endpoint::try_from("http://[::]:0")?
        .connect_with_connector(service_fn(|_: Uri| future::ready(VmSocket::connect(7070))))
        .await?;

    let dbus_connection = ConnectionBuilder::session()?
        .internal_executor(false)
        .build()
        .await?;

    // start up a task for the zbus executor to use.
    let handle = {
        // need to clone connection before moving into async task.
        let dbus_connection = dbus_connection.clone();
        tokio::spawn(async move {
            loop {
                dbus_connection.executor().tick().await;
            }
        })
    };

    services::init_all(grpc_channel, &dbus_connection).await?;

    handle.await?;

    Ok(())
}
