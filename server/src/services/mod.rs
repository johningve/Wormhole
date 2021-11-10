use zbus::Connection;

use self::filechooser::FileChooser;

pub mod filechooser;

pub const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";

pub async fn init_all(connection: &Connection, distro: &str) -> zbus::Result<()> {
    FileChooser::init(connection, distro).await?;

    Ok(())
}
