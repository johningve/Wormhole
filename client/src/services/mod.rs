use tonic::transport::Channel;
use zbus::{Connection, ObjectServer};

use self::notifications::Notifications;

pub mod notifications;

pub fn init_all(
    grpc_channel: Channel,
    dbus_connection: &Connection,
    object_server: &mut ObjectServer,
) -> zbus::Result<()> {
    Notifications::init(grpc_channel, dbus_connection, object_server)?;

    Ok(())
}
