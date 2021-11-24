use std::{
    collections::HashSet,
    convert::TryFrom,
    sync::{Arc, Mutex},
};

use futures::{FutureExt, StreamExt};
use zbus::{
    dbus_interface, fdo,
    names::{BusName, OwnedBusName},
    Connection, InterfaceDeref, SignalContext,
};

use crate::unwrap_or_log;

const PATH: &str = "/StatusNotifierWatcher";

#[derive(Default)]
struct WatcherInner {
    items: HashSet<OwnedBusName>,
    host: Option<OwnedBusName>,
}

#[derive(Clone)]
pub struct StatusNotifierWatcher {
    inner: Arc<Mutex<WatcherInner>>,
}

impl StatusNotifierWatcher {
    pub async fn init(connection: &Connection, _distro: &str) -> zbus::Result<()> {
        let watcher = StatusNotifierWatcher {
            inner: Arc::new(Mutex::new(WatcherInner::default())),
        };

        watcher.handle_name_owner_changed(connection).await?;

        connection.object_server_mut().await.at(PATH, watcher)?;

        Ok(())
    }

    fn insert_item(&self, service: BusName<'_>) -> bool {
        let mut inner = self.inner.lock().unwrap();
        inner.items.insert(service.into())
    }

    fn remove_item(&self, service: BusName<'_>) -> bool {
        let mut inner = self.inner.lock().unwrap();
        inner.items.remove(&service.into())
    }

    fn register_host(&self, service: BusName<'_>) {
        let mut inner = self.inner.lock().unwrap();
        inner.host = Some(service.into());
    }

    fn is_host_registered(&self) -> bool {
        let mut inner = self.inner.lock().unwrap();
        inner.host.is_some()
    }

    async fn handle_name_owner_changed(&self, connection: &Connection) -> zbus::Result<()> {
        let watcher = self.clone();
        let connection = connection.clone();
        let dbus = fdo::DBusProxy::new(&connection).await?;
        let mut name_owner_changed_stream = dbus.receive_name_owner_changed().await?;

        tokio::spawn(async move {
            while let Some(signal) = name_owner_changed_stream.next().await {
                let args = unwrap_or_log!(signal.args());

                let name = args.name();
                let old_owner = args.old_owner();

                if old_owner.is_some() {
                    connection
                        .object_server()
                        .await
                        .with(
                            PATH,
                            |_iface: InterfaceDeref<'_, StatusNotifierWatcher>, ctx| async move {
                                StatusNotifierWatcher::status_notifier_item_unregistered(
                                    &ctx,
                                    name.clone(),
                                )
                                .await
                            },
                        )
                        .await
                        .unwrap_or_else(|err| log::error!("{}", err))
                }
            }
        });

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
        let bus_name = unwrap_or_log!(BusName::try_from(service));

        if self.insert_item(bus_name) {
            Self::status_notifier_item_registered(&ctx, service)
                .await
                .unwrap_or_else(|err| log::error!("{}", err));
        }
    }

    async fn register_status_notifier_host(&self, service: BusName<'_>) {
        self.register_host(service);
    }

    #[dbus_interface(property)]
    fn registered_status_notifier_items(&self) -> Vec<&'_ str> {
        let inner = self.inner.lock().unwrap();
        inner.items.iter().map(|n| n.as_str()).collect()
    }

    #[dbus_interface(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        self.is_host_registered()
    }

    #[dbus_interface(property)]
    fn protocol_version(&self) -> i32 {
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
