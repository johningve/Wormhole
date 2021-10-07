use futures::TryFutureExt;
use rpc::notifications::Action;
use rpc::notifications::{
    notifications_client::NotificationsClient, notify_response::Event, CloseNotificationRequest,
    NotifyRequest,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tonic::codegen::InterceptedService;
use tonic::transport::Channel;
use zbus::{dbus_interface, Connection};
use zbus::{zvariant, SignalContext};
use zvariant_derive::Type;

use super::interceptor::DistroInterceptor;

#[derive(Clone)]
pub struct Notifications {
    remote: NotificationsClient<InterceptedService<Channel, DistroInterceptor>>,
}

impl Notifications {
    pub async fn init(grpc_channel: Channel, dbus_connection: &Connection) -> zbus::Result<()> {
        // register name
        dbus_connection
            .request_name("org.freedesktop.Notifications")
            .await?;

        // register service
        dbus_connection.object_server_mut().await.at(
            "/org/freedesktop/Notifications",
            Notifications {
                remote: NotificationsClient::with_interceptor(grpc_channel, DistroInterceptor {}),
            },
        )?;

        Ok(())
    }
}

fn get_icon_path(icon_name: &str) -> Option<String> {
    log::debug!("looking up icon: {}", icon_name);

    if let Ok(md) = std::fs::metadata(icon_name) {
        if md.is_file() {
            return Some(icon_name.to_string());
        }
    }

    if let Some(Ok(icon)) = linicon::lookup_icon(icon_name).with_size(64).next() {
        let icon_path = icon.path.to_string_lossy().to_string();
        log::debug!("found icon for {}: {}", icon_name, icon_path);
        return Some(icon_path);
    }

    None
}

#[dbus_interface(name = "org.freedesktop.Notifications")]
impl Notifications {
    async fn close_notification(&self, id: u32) {
        log::debug!("close_notification {:#?}", id);

        let mut s = self.clone();
        s.remote
            .close_notification(tonic::Request::new(CloseNotificationRequest { id }))
            .await
            .map_err(|err| log::error!("{}", err))
            .unwrap();
    }

    fn get_capabilities(&self) -> Vec<String> {
        log::debug!("get_capabilities");
        Vec::new()
    }

    fn get_server_information(&self) -> ServerInformation {
        log::debug!("get_server_information");
        ServerInformation::get()
    }

    async fn notify(
        &self,
        #[zbus(signal_context)] ctx: SignalContext<'_>,
        notification: Notification<'_>,
    ) -> u32 {
        log::debug!("notify {:#?}", notification);
        let (tx, mut rx) = mpsc::channel(1);
        let mut s = self.clone();
        let request = NotifyRequest {
            app_name: notification.app_name.into(),
            replaces_id: notification.replaces_id,
            image_path: get_icon_path(notification.app_icon).unwrap_or_default(),
            summary: notification.summary.into(),
            body: notification.body.into(),
            actions: notification
                .actions
                .chunks_exact(2)
                .map(|s| Action {
                    name: s[0].to_string(),
                    label: s[1].to_string(),
                })
                .collect(),
            expire_timeout: notification.expire_timeout,
        };

        let mut stream = s
            .remote
            .notify(tonic::Request::new(request))
            .await
            .map_err(|err| log::error!("{}", err))
            .unwrap()
            .into_inner();

        let ctx = Box::pin(SignalContext::new(ctx.connection(), ctx.path().to_owned()).unwrap());

        tokio::spawn(async move {
            while let Some(response) = stream
                .message()
                .await
                .map_err(|err| log::error!("{}", err))
                .unwrap()
            {
                if let Some(event) = response.event {
                    match event {
                        Event::Created(e) => tx.send(e.id).await.unwrap(),

                        Event::Dismissed(e) => Self::notification_closed(&ctx, e.id, 0)
                            .map_err(|err| log::error!("{}", err))
                            .await
                            .unwrap(),
                        Event::ActionInvoked(e) => {
                            Self::action_invoked(&ctx, e.id, e.action.as_str())
                                .map_err(|err| log::error!("{}", err))
                                .await
                                .unwrap()
                        }
                    };
                }
            }
        });

        rx.recv().await.unwrap()
    }

    #[dbus_interface(signal)]
    async fn notification_closed(ctx: &SignalContext<'_>, id: u32, reason: u32)
        -> zbus::Result<()>;

    #[dbus_interface(signal)]
    async fn action_invoked(ctx: &SignalContext<'_>, id: u32, action_key: &str)
        -> zbus::Result<()>;
}

#[derive(Debug, Type, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Type, Serialize, Deserialize)]
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
