use zbus::{dbus_interface, fdo, Connection};

pub struct WSL {}

impl WSL {
    pub async fn init(connection: &Connection) -> zbus::Result<()> {
        connection.request_name("com.github.raytar.WSL").await?;

        connection
            .object_server_mut()
            .await
            .at("/com/github/raytar/WSL", WSL {})?;

        Ok(())
    }
}

#[dbus_interface(name = "com.github.raytar.WSL")]
impl WSL {
    fn distro_name(&self) -> fdo::Result<String> {
        std::env::var("WSL_DISTRO_NAME").map_err(|e| fdo::Error::Failed(e.to_string()))
    }

    fn user_name(&self) -> String {
        whoami::username()
    }

    fn user_home(&self) -> fdo::Result<String> {
        std::env::var("HOME").map_err(|e| fdo::Error::Failed(e.to_string()))
    }
}
