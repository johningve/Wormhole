use zbus::dbus_proxy;

#[dbus_proxy(
    interface = "org.kde.StatusNotifierWatcher",
    default_service = "org.kde.StatusNotifierWatcher",
    default_path = "/StatusNotifierWatcher"
)]
pub trait StatusNotifierWatcher {
    #[dbus_proxy(property)]
    fn registered_status_notifier_items(&self) -> zbus::Result<Vec<String>>;

    #[dbus_proxy(property)]
    fn is_status_notifier_host_registered(&self) -> zbus::Result<bool>;

    #[dbus_proxy(property)]
    fn protocol_version(&self) -> zbus::Result<i32>;

    fn register_status_notifier_item(&self, service: &str) -> zbus::Result<()>;

    fn register_status_notifier_host(&self, service: &str) -> zbus::Result<()>;

    #[dbus_proxy(signal)]
    fn status_notifier_item_registered(&self, service: &str);

    #[dbus_proxy(signal)]
    fn status_notifier_item_unregistered(&self, service: &str);

    #[dbus_proxy(signal)]
    fn status_notifier_host_registered(&self);
}
