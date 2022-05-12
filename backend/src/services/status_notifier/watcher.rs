// Copyright (c) 2022 John Ingve Olsen
//
// This software is released under the MIT License.
// https://opensource.org/licenses/MIT

use std::convert::TryFrom;

use futures::StreamExt;

use zbus::{dbus_interface, fdo, names::BusName, Connection, MessageHeader, SignalContext};
use zvariant::ObjectPath;

use crate::proxies::status_notifier_item::StatusNotifierItemProxy;

use super::host::StatusNotifierHost;

const PATH: &str = "/StatusNotifierWatcher";

#[derive(Clone)]
pub struct StatusNotifierWatcher {
    host: StatusNotifierHost,
}

impl StatusNotifierWatcher {
    pub async fn init(connection: &Connection) -> anyhow::Result<()> {
        let watcher = StatusNotifierWatcher {
            host: StatusNotifierHost::new().await?,
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

    async fn handle_name_owner_changed(self, connection: Connection) -> zbus::Result<()> {
        let dbus = fdo::DBusProxy::new(&connection).await?;
        let mut name_owner_changed_stream = dbus.receive_name_owner_changed().await?;

        while let Some(signal) = name_owner_changed_stream.next().await {
            let args = signal.args()?;

            log::debug!(
                "name_owner_changed ({}): old: {}, new: {}",
                args.name(),
                if let Some(old) = args.old_owner().as_deref() {
                    old.to_string()
                } else {
                    String::from("None")
                },
                if let Some(new) = args.new_owner().as_deref() {
                    new.to_string()
                } else {
                    String::from("None")
                },
            );

            let name = args.name();

            if args.old_owner().is_some() {
                let removed = self.host.handle_service_disappeared(name);
                let iface = connection
                    .object_server()
                    .interface::<_, StatusNotifierWatcher>(PATH)
                    .await?;
                for indicator in removed {
                    StatusNotifierWatcher::status_notifier_item_unregistered(
                        iface.signal_context(),
                        &indicator,
                    )
                    .await?;
                }
            }
        }

        Ok(())
    }
}

#[dbus_interface(name = "org.kde.StatusNotifierWatcher")]
impl StatusNotifierWatcher {
    async fn register_status_notifier_item(
        &self,
        #[zbus(header)] hdr: MessageHeader<'_>,
        #[zbus(signal_context)] ctx: SignalContext<'_>,
        service: &str,
    ) -> fdo::Result<()> {
        log::debug!("register_status_notifier_item: {}", service);

        let mut object_path = ObjectPath::from_str_unchecked("/StatusNotifierItem");
        let bus_name = if let Ok(name) = BusName::try_from(service) {
            name
        } else if let Some(sender) = hdr.sender()? {
            if let Ok(path) = ObjectPath::try_from(service) {
                object_path = path;
            }
            BusName::Unique(sender.clone())
        } else {
            return Err(fdo::Error::Failed(String::from(
                "Could not determine bus name",
            )));
        };

        let proxy = StatusNotifierItemProxy::builder(ctx.connection())
            .destination(bus_name.into_owned())?
            .path(object_path.into_owned())?
            .build()
            .await?;

        match self.host.insert_item(proxy) {
            Ok(v) => {
                Self::status_notifier_item_registered(&ctx, service).await?;
                Self::registered_status_notifier_items_changed(self, &ctx).await?;
                Ok(())
            }
            Err(e) => {
                log::error!("register_status_notifier_item failed: {}", e.to_string());
                Err(fdo::Error::Failed(e.to_string()))
            }
        }
    }

    async fn register_status_notifier_host(&self, _service: BusName<'_>) -> fdo::Result<()> {
        Err(fdo::Error::NotSupported(String::from(
            "Registering additional notifier hosts is not supported.",
        )))
    }

    #[dbus_interface(property)]
    fn registered_status_notifier_items(&self) -> Vec<String> {
        self.host.registered_items()
    }

    #[dbus_interface(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        true
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
