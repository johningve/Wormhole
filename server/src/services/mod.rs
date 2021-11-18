use zbus::Connection;

use self::{filechooser::FileChooser, notifications::Notifications};

pub mod filechooser;
pub mod notifications;
pub mod statusnotifierwatcher;

pub const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";

pub async fn init_all(connection: &Connection, distro: &str) -> zbus::Result<()> {
    FileChooser::init(connection, distro).await?;
    Notifications::init(connection, distro).await?;

    Ok(())
}
