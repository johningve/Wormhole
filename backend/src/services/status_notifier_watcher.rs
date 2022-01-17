use std::{
    collections::HashSet,
    convert::TryFrom,
    sync::{Arc, Mutex},
};

use futures::StreamExt;
use zbus::{
    dbus_interface, fdo,
    names::{BusName, OwnedBusName},
    Connection, InterfaceDeref, SignalContext,
};

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
    pub async fn init(connection: &Connection) -> zbus::Result<()> {
        let watcher = StatusNotifierWatcher {
            inner: Arc::new(Mutex::new(WatcherInner::default())),
        };

        {
            let connection = connection.clone();
            let watcher = watcher.clone();
            tokio::spawn(async move {
                Self::handle_name_owner_changed(watcher, connection)
                    .await
                    .unwrap_or_else(|e| log::error!("{}", e))
            });
        }

        connection.object_server().at(PATH, watcher).await?;

        Ok(())
    }

    fn insert_item(&self, service: BusName<'_>) -> bool {
        let mut inner = self.inner.lock().unwrap();
        inner.items.insert(service.into())
    }

    fn remove_item(&self, service: BusName<'_>) -> bool {
        let mut inner = self.inner.lock().unwrap();
        inner.items.remove(&OwnedBusName::from(service))
    }

    fn register_host(&self, service: BusName<'_>) {
        let mut inner = self.inner.lock().unwrap();
        inner.host = Some(service.into());
    }

    fn is_host_registered(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        inner.host.is_some()
    }

    async fn handle_name_owner_changed(self, connection: Connection) -> zbus::Result<()> {
        let dbus = fdo::DBusProxy::new(&connection).await?;
        let mut name_owner_changed_stream = dbus.receive_name_owner_changed().await?;

        while let Some(signal) = name_owner_changed_stream.next().await {
            let args = signal.args()?;

            let name = args.name();

            if args.old_owner().is_some() && self.remove_item(BusName::try_from(name)?) {
                let iface = connection
                    .object_server()
                    .interface::<_, StatusNotifierWatcher>(PATH)
                    .await?;
                StatusNotifierWatcher::status_notifier_item_unregistered(
                    iface.signal_context(),
                    name.as_str(),
                )
                .await?;
            }
        }

        Ok(())
    }
}

#[dbus_interface(name = "org.freedesktop.StatusNotifierWatcher")]
impl StatusNotifierWatcher {
    async fn register_status_notifier_item(
        &self,
        #[zbus(signal_context)] ctx: SignalContext<'_>,
        service: &str,
    ) -> fdo::Result<()> {
        let bus_name = BusName::try_from(service).map_err(|e| fdo::Error::Failed(e.to_string()))?;

        if self.insert_item(bus_name) {
            Self::status_notifier_item_registered(&ctx, service).await?;
            Self::registered_status_notifier_items_changed(self, &ctx).await?;
        }

        Ok(())
    }

    async fn register_status_notifier_host(
        &self,
        #[zbus(signal_context)] ctx: SignalContext<'_>,
        service: BusName<'_>,
    ) -> fdo::Result<()> {
        self.register_host(service);
        Self::is_status_notifier_host_registered_changed(self, &ctx).await?;

        Ok(())
    }

    #[dbus_interface(property)]
    fn registered_status_notifier_items(&self) -> Vec<String> {
        let inner = self.inner.lock().unwrap();
        inner.items.iter().map(|n| n.to_string()).collect()
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
