use tonic::transport::Channel;
use zbus::Connection;

use self::notifications::Notifications;

pub mod interceptor;
pub mod notifications;

pub async fn init_all(grpc_channel: Channel, dbus_connection: &Connection) -> zbus::Result<()> {
    Notifications::init(grpc_channel, dbus_connection).await?;

    Ok(())
}
