use zbus::dbus_proxy;

#[dbus_proxy(
    interface = "com.github.raytar.Icons",
    default_service = "com.github.raytar.Icons",
    default_path = "/com/github/raytar/Icons"
)]
pub trait Icons {
    fn lookup_icon(&self, icon: &str, size: u16) -> zbus::Result<String>;
}
