use zbus::{dbus_interface, SignalContext};

pub struct StatusNotifierWatcher {}

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
    ) -> zbus::Result<()> {
    }

    #[dbus_interface(signal)]
    async fn status_notifier_host_registered(ctxt: &SignalContext<'_>) -> zbus::Result<()> {}
}
