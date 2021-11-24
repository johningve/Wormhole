use zbus::blocking::Connection;

pub struct StatusNotifierHost {}

impl StatusNotifierHost {
    async fn init(connection: &Connection, distro_name: &str) -> zbus::Result<()> {}
}
