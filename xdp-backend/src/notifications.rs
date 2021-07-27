use serde::{Deserialize, Serialize};
use zbus::dbus_interface;
use zvariant_derive::Type;

struct Notifications {}

#[dbus_interface(name = "org.freedesktop.Notifications")]
impl Notifications {
    fn close_notification(&self, id: u32) {
        println!("CloseNotification called with id {}", id);
    }

    fn get_capabilities(&self) -> Vec<String> {
        println!("GetCapabilities called");
        return Vec::new();
    }

    fn get_server_information(&self) -> ServerInformation {
        println!("GetServerInformation called");
        return ServerInformation::get();
    }
}

#[derive(Debug, Type, Serialize, Deserialize)]
pub struct ServerInformation {
    /// The product name of the server.
    pub name: String,

    /// The vendor name. For example "KDE," "GNOME," "freedesktop.org" or "Microsoft".
    pub vendor: String,

    /// The server's version number.
    pub version: String,

    /// The specification version the server is compliant with.
    pub spec_version: String,
}

impl ServerInformation {
    fn get() -> ServerInformation {
        return ServerInformation {
            name: String::from("master"),
            vendor: String::from("John Ingve Olsen"),
            version: String::from("0.1"),
            spec_version: String::from("1.2"),
        };
    }
}
