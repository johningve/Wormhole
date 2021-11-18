use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use futures::FutureExt;
use zbus::{dbus_interface, fdo, Connection, InterfaceDeref, SignalContext};

const PATH: &str = "/StatusNotifierWatcher";

#[derive(Clone)]
pub struct StatusNotifierWatcher {
    items: Arc<Mutex<HashSet<String>>>,
}

impl StatusNotifierWatcher {
    pub async fn init(connection: &Connection, _distro: &str) -> zbus::Result<()> {
        let watcher = StatusNotifierWatcher {
            items: Arc::new(Mutex::new(HashSet::new())),
        };

        watcher.handle_name_owner_changed(connection).await?;

        connection.object_server_mut().await.at(PATH, watcher)?;

        Ok(())
    }

    fn insert_item(&self, service: &str) -> bool {
        let mut items = self.items.lock().unwrap();
        items.insert(service.to_string())
    }

    fn remove_item(&self, service: &str) -> bool {
        let mut items = self.items.lock().unwrap();
        items.remove(service)
    }

    async fn handle_name_owner_changed(&self, connection: &Connection) -> zbus::Result<()> {
        let watcher = self.clone();
        let connection = connection.clone();
        let dbus = fdo::DBusProxy::new(&connection).await?;
        dbus.connect_name_owner_changed(move |name, _old_owner, new_owner| {
            if new_owner.is_none() {
                // service was unregistered
                if watcher.remove_item(name.as_str()) {
                    let connection = connection.clone(); // rust gets very angry if I don't copy this again.
                    return async move {
                        connection
                        .object_server()
                        .await
                        .with(
                            PATH,
                            |_iface: InterfaceDeref<'_, StatusNotifierWatcher>, ctx| async move {
                                StatusNotifierWatcher::status_notifier_item_unregistered(
                                    &ctx,
                                    name.as_str(),
                                )
                                .await
                            },
                        )
                        .await.unwrap_or_else(|err| log::error!("{}", err));
                    }
                    .boxed();
                }
            }
            futures::future::ready(()).boxed()
        })
        .await?;

        Ok(())
    }
}

#[dbus_interface(name = "org.freedesktop.StatusNotifierWatcher")]
impl StatusNotifierWatcher {
    async fn register_status_notifier_item(
        &self,
        #[zbus(signal_context)] ctx: SignalContext<'_>,
        service: &str,
    ) {
        if self.insert_item(service) {
            Self::status_notifier_item_registered(&ctx, service)
                .await
                .unwrap_or_else(|err| log::error!("{}", err));
        }
    }

    async fn register_status_notifier_host(&self, _service: &str) {}

    #[dbus_interface(property)]
    async fn registered_status_notifier_items(&self) -> Vec<String> {
        let items = self.items.lock().unwrap();
        items.iter().map(String::from).collect()
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
    ) -> zbus::Result<()>;

    #[dbus_interface(signal)]
    async fn status_notifier_item_unregistered(
        ctxt: &SignalContext<'_>,
        service: &str,
    ) -> zbus::Result<()>;

    #[dbus_interface(signal)]
    async fn status_notifier_host_registered(ctxt: &SignalContext<'_>) -> zbus::Result<()>;
}
