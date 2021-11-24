use zbus::Connection;

use crate::proxies::status_notifier_watcher::StatusNotifierWatcherProxy;

pub struct StatusNotifierHost {
    distro_name: String,
}

impl StatusNotifierHost {
    pub async fn init(connection: &Connection) -> zbus::Result<()> {
        let watcher_proxy = StatusNotifierWatcherProxy::new(connection).await?;

        let item_registered_stream = watcher_proxy
            .receive_status_notifier_item_registered()
            .await?;

        let item_unregistered_stream = watcher_proxy
            .receive_status_notifier_item_unregistered()
            .await?;

        Ok(())
    }
}
