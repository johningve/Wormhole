use futures::future;
use std::{convert::TryFrom, error::Error};
use tonic::transport::{Endpoint, Uri};
use tower::service_fn;
use vmsocket::VmSocket;
use zbus::{Connection, ObjectServer};

mod services;
mod vmsocket;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let grpc_channel = Endpoint::try_from("http://[::]:0")?
        .connect_with_connector(service_fn(|_: Uri| future::ready(VmSocket::connect(7070))))
        .await?;

    let dbus_connection = Connection::new_session()?;
    let mut object_server = ObjectServer::new(&dbus_connection);

    services::init_all(grpc_channel, &dbus_connection, &mut object_server)?;

    loop {
        if let Err(err) = object_server.try_handle_next() {
            eprintln!("{}", err);
        }
    }
}
