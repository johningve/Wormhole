use std::{collections::HashSet, sync::Mutex};

use futures::{Future, FutureExt};
use zbus::{
    dbus_interface, fdo,
    names::{BusName, UniqueName},
    Connection, SignalContext,
};
use zvariant::Optional;

#[derive(Default)]
struct StatusNotifierWatcherData {
    items: HashSet<String>,
}

#[derive(Clone)]
pub struct StatusNotifierWatcher {
    data: Mutex<StatusNotifierWatcherData>,
}

impl StatusNotifierWatcher {
    pub async fn init(connection: &Connection, distro: &str) -> zbus::Result<()> {
        let mut watcher = StatusNotifierWatcher {
            data: Mutex::new(StatusNotifierWatcherData::default()),
        };

        let dbus = fdo::DBusProxy::new(connection).await?;
        dbus.connect_name_owner_changed(|name, old_owner, new_owner| {
            let watcher = Box::new(watcher.clone());
            watcher
                .handle_name_owner_changed(name, old_owner, new_owner)
                .boxed()
        })
        .await?;
        Ok(())
    }

    async fn handle_name_owner_changed(
        &self,
        name: BusName<'_>,
        old_owner: Optional<UniqueName<'_>>,
        new_owner: Optional<UniqueName<'_>>,
    ) {
    }
}

#[dbus_interface(name = "org.freedesktop.StatusNotifierWatcher")]
impl StatusNotifierWatcher {
    async fn register_status_notifier_item(
        &self,
        #[zbus(signal_context)] ctx: SignalContext<'_>,
        service: &str,
    ) {
    }

    async fn register_status_notifier_host(&self, _service: &str) {}

    #[dbus_interface(property)]
    async fn registered_status_notifier_items(&self) -> Vec<&str> {
        Vec::new()
    }

    #[dbus_interface(property)]
    async fn is_status_notifier_host_registered(&self) -> bool {
        true
    }

    #[dbus_interface(property)]
    async fn protocol_version(&self) -> i32 {
        0
    }

    #[dbus_interface(signal)]
    async fn status_notifier_item_registered(
        ctxt: &SignalContext<'_>,
        service: &str,
    ) -> zbus::Result<()> {
    }

    #[dbus_interface(signal)]
    async fn status_notifier_item_unregistered(
        ctxt: &SignalContext<'_>,
        service: &str,
    ) -> zbus::Result<()>;

    #[dbus_interface(signal)]
    async fn status_notifier_host_registered(ctxt: &SignalContext<'_>) -> zbus::Result<()>;
}
