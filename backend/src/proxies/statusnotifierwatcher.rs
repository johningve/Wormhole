use zbus::{
    dbus_proxy, fdo,
    names::{BusName, OwnedBusName, OwnedWellKnownName},
};

#[dbus_proxy(
    interface = "org.freedesktop.StatusNotifierWatcher",
    default_service = "org.freedesktop.StatusNotifierWatcher",
    default_path = "/StatusNotifierWatcher"
)]
pub trait StatusNotifierWatcher {
    #[dbus_proxy(property)]
    fn registered_status_notifier_items(&self) -> fdo::Result<Vec<OwnedWellKnownName>>;

    fn register_status_notifier_item(&self, service: BusName<'_>) -> zbus::Result<()>;

    fn register_status_notifier_host(&self, service: BusName<'_>) -> zbus::Result<()>;
}
