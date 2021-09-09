use rpc::notifications::notifications_client::NotificationsClient;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, convert::TryFrom};
use tonic::transport::Channel;
use zbus::{dbus_interface, fdo, Connection, ObjectServer};
use zvariant::ObjectPath;
use zvariant_derive::Type;

pub struct Notifications {
    remote: NotificationsClient<Channel>,
}

impl Notifications {
    pub fn init(
        grpc_channel: Channel,
        dbus_connection: &Connection,
        object_server: &mut ObjectServer,
    ) -> zbus::Result<()> {
        // register name
        fdo::DBusProxy::new(dbus_connection)?.request_name(
            "org.freedesktop.Notifications",
            fdo::RequestNameFlags::ReplaceExisting.into(),
        )?;

        // register service
        object_server.at(
            &ObjectPath::try_from("/org/freedesktop/Notifications")?,
            Notifications {
                remote: NotificationsClient::new(grpc_channel),
            },
        )?;

        Ok(())
    }
}

#[dbus_interface(name = "org.freedesktop.Notifications")]
impl Notifications {
    fn close_notification(&self, id: u32) {
        println!("CloseNotification called with id {:#?}", id);
    }

    fn get_capabilities(&self) -> Vec<String> {
        println!("GetCapabilities called");
        Vec::new()
    }

    fn get_server_information(&self) -> ServerInformation {
        println!("GetServerInformation called");
        ServerInformation::get()
    }

    fn notify(&self, notification: Notification) -> u32 {
        println!("Notify called with notification {:#?}", notification);
        0
    }

    #[dbus_interface(signal)]
    fn notification_closed(&self, id: u32, reason: u32) -> zbus::Result<()>;

    #[dbus_interface(signal)]
    fn action_invoked(&self, id: u32, action_key: &str) -> zbus::Result<()>;
}

#[derive(Debug, Type, Serialize)]
pub struct ServerInformation<'a> {
    /// The product name of the server.
    pub name: &'a str,

    /// The vendor name. For example "KDE," "GNOME," "freedesktop.org" or "Microsoft".
    pub vendor: &'a str,

    /// The server's version number.
    pub version: &'a str,

    /// The specification version the server is compliant with.
    pub spec_version: &'a str,
}

impl<'a> ServerInformation<'_> {
    fn get() -> Self {
        Self {
            name: "master",
            vendor: "John Ingve Olsen",
            version: "0.1",
            spec_version: "1.2",
        }
    }
}

#[derive(Clone, Debug, Type, Deserialize)]
pub struct Notification<'a> {
    pub app_name: &'a str,
    pub replaces_id: u32,
    pub app_icon: &'a str,
    pub summary: &'a str,
    pub body: &'a str,
    pub actions: Vec<&'a str>,
    pub hints: HashMap<&'a str, zvariant::Value<'a>>,
    pub expire_timeout: i32,
}
