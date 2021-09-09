use rpc::notifications::{
    notifications_client::NotificationsClient, notify_response::Event, CloseNotificationRequest,
    NotifyRequest,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, convert::TryFrom, sync::mpsc::sync_channel};
use tonic::transport::Channel;
use zbus::{dbus_interface, fdo, Connection, ObjectServer};
use zvariant::ObjectPath;
use zvariant_derive::Type;

#[derive(Clone)]
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
        log::debug!("close_notification {:#?}", id);

        let mut s = self.clone();
        tokio::spawn(async move {
            s.remote
                .close_notification(tonic::Request::new(CloseNotificationRequest { id }))
                .await
                .map_err(|err| log::error!("{}", err))
                .unwrap();
        });
    }

    fn get_capabilities(&self) -> Vec<String> {
        log::debug!("get_capabilities");
        Vec::new()
    }

    fn get_server_information(&self) -> ServerInformation {
        log::debug!("get_server_information");
        ServerInformation::get()
    }

    fn notify(&self, notification: Notification) -> u32 {
        log::debug!("notify {:#?}", notification);
        let (tx, rx) = sync_channel(1);
        let mut s = self.clone();
        let request = NotifyRequest {
            app_name: notification.app_name.into(),
            replaces_id: notification.replaces_id,
            app_icon: notification.app_icon.into(),
            summary: notification.summary.into(),
            body: notification.body.into(),
            actions: notification.actions.iter().map(|s| s.to_string()).collect(),
            expire_timeout: notification.expire_timeout,
        };

        tokio::spawn(async move {
            let mut stream = s
                .remote
                .notify(tonic::Request::new(request))
                .await
                .map_err(|err| log::error!("{}", err))
                .unwrap()
                .into_inner();

            while let Some(response) = stream
                .message()
                .await
                .map_err(|err| log::error!("{}", err))
                .unwrap()
            {
                if let Some(event) = response.event {
                    match event {
                        Event::Created(e) => tx.send(e.id).unwrap(),

                        Event::Dismissed(e) => s
                            .notification_closed(e.id, 0)
                            .map_err(|err| log::error!("{}", err))
                            .unwrap(),
                        Event::ActionInvoked(e) => s
                            .action_invoked(e.id, e.action.as_str())
                            .map_err(|err| log::error!("{}", err))
                            .unwrap(),
                    };
                }
            }
        });

        rx.recv().unwrap()
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
