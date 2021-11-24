use zbus::Connection;

use self::{
    filechooser::FileChooser, notifications::Notifications,
    status_notifier_host::StatusNotifierHost, status_notifier_watcher::StatusNotifierWatcher,
};

pub mod filechooser;
pub mod notifications;
pub mod status_notifier_host;
pub mod status_notifier_watcher;

pub const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";

pub async fn init_all(connection: &Connection) -> zbus::Result<()> {
    FileChooser::init(connection).await?;
    Notifications::init(connection).await?;
    StatusNotifierWatcher::init(connection).await?;
    StatusNotifierHost::init(connection).await?;

    Ok(())
}
