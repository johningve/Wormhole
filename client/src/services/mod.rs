use tonic::transport::Channel;
use zbus::Connection;
use zvariant::dbus;

use self::{filechooser::FileChooser, notifications::Notifications};

pub mod filechooser;
pub mod interceptor;
pub mod notifications;

pub const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";

pub async fn init_all(grpc_channel: Channel, dbus_connection: &Connection) -> zbus::Result<()> {
    Notifications::init(grpc_channel.clone(), dbus_connection).await?;
    FileChooser::init(grpc_channel, dbus_connection).await?;

    Ok(())
}
