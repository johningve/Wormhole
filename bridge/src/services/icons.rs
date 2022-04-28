use std::path::Path;
use zbus::{dbus_interface, Connection};

pub struct Icons {}

impl Icons {
    pub async fn init(connection: &Connection) -> zbus::Result<()> {
        connection.request_name("com.github.raytar.Icons").await?;

        connection
            .object_server_mut()
            .await
            .at("/com/github/raytar/Icons", Icons {})?;

        Ok(())
    }
}

#[dbus_interface(name = "com.github.raytar.Icons")]
impl Icons {
    fn lookup_icon(&self, icon: &str, size: u16) -> String {
        log::debug!("looking up icon: {}", icon);

        // first check if icon is a path to an existing icon
        let path = Path::new(icon);
        if path.is_absolute() && path.is_file() {
            return icon.to_string();
        }

        if let Some(Ok(icon_path)) = linicon::lookup_icon(icon).with_size(size).next() {
            let icon_path = icon_path.path.to_string_lossy().to_string();
            log::debug!("found icon for {}: {}", icon, icon_path);
            return icon_path;
        }

        String::new()
    }
}
