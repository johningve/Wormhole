mod toasthelper;

use std::{
    collections::{BTreeMap, HashMap},
    sync::Mutex,
};

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use windows::UI::Notifications::ToastDismissalReason;
use zbus::{dbus_interface, Connection, SignalContext};
use zvariant_derive::Type;

use self::toasthelper::ToastHelper;
use crate::proxies::icons::IconsProxy;

enum ToastEvent {
    Activated(String),
    Dismissed(ToastDismissalReason),
    Failed(windows::core::Error),
}

enum NotificationClosedReason {
    Expired = 1,
    Dismissed,
    Closed,
    Undefined,
}

#[derive(Default)]
struct NotificationsServiceData {
    next_id: u32,
    notifications: BTreeMap<u32, ToastHelper>,
}

pub struct Notifications {
    icons: Box<IconsProxy<'static>>,
    data: Mutex<NotificationsServiceData>,
}

impl Notifications {
    pub async fn init(connection: &Connection) -> zbus::Result<()> {
        connection
            .request_name("org.freedesktop.Notifications")
            .await?;

        connection
            .object_server()
            .at(
                "/org/freedesktop/Notifications",
                Notifications {
                    icons: Box::new(IconsProxy::new(connection).await?),
                    data: Mutex::new(NotificationsServiceData {
                        next_id: 1,
                        notifications: BTreeMap::new(),
                    }),
                },
            )
            .await?;

        log::info!("org.freedesktop.Notifications server enabled");

        Ok(())
    }

    async fn notify_internal(
        &self,
        ctx: SignalContext<'_>,
        notification: Notification<'_>,
    ) -> anyhow::Result<u32> {
        let icon = self.icons.lookup_icon(notification.app_icon, 64).await?;

        let mut data = self.data.lock().expect("poisoned mutex");

        let id = data.next_id;
        data.next_id += 1;

        let toast = ToastHelper::new(
            &id.to_string(),
            notification.summary,
            notification.body,
            if !icon.is_empty() { Some(&icon) } else { None }, // cool
            notification.actions,
        )?;

        let (tx, mut rx) = mpsc::unbounded_channel();

        {
            let tx = tx.clone();
            toast.on_activated(move |action| {
                tx.send(ToastEvent::Activated(action))
                    .unwrap_or_else(|err| log::error!("failed to send toast event: {}", err))
            })?;
        }

        {
            let tx = tx.clone();
            toast.on_dismissed(move |reason| {
                tx.send(ToastEvent::Dismissed(reason))
                    .unwrap_or_else(|err| log::error!("failed to send toast event: {}", err))
            })?;
        }

        {
            toast.on_failed(move |err| {
                tx.send(ToastEvent::Failed(err))
                    .unwrap_or_else(|err| log::error!("failed to send toast event: {}", err))
            })?;
        }

        let ctx = SignalContext::from_parts(ctx.connection().clone(), ctx.path().to_owned());

        tokio::spawn(async move {
            if let Some(event) = rx.recv().await {
                match event {
                    ToastEvent::Activated(action) => Self::action_invoked(&ctx, id, &action).await,
                    ToastEvent::Dismissed(reason) => {
                        let reason = if reason == ToastDismissalReason::ApplicationHidden {
                            NotificationClosedReason::Closed
                        } else if reason == ToastDismissalReason::TimedOut {
                            NotificationClosedReason::Expired
                        } else if reason == ToastDismissalReason::UserCanceled {
                            NotificationClosedReason::Dismissed
                        } else {
                            NotificationClosedReason::Undefined
                        };

                        Self::notification_closed(&ctx, id, reason as _).await
                    }
                    ToastEvent::Failed(err) => {
                        log::error!("toast notification failed: {}", err);
                        Self::notification_closed(
                            &ctx,
                            id,
                            NotificationClosedReason::Undefined as _,
                        )
                        .await
                    }
                }
                .unwrap_or_else(|err| log::error!("failed to send notification signal: {}", err))
            }
        });

        toast.show()?;

        Ok(id)
    }
}

#[dbus_interface(name = "org.freedesktop.Notifications")]
impl Notifications {
    async fn close_notification(&self, id: u32) {
        log::debug!("close_notification {:#?}", id);

        let mut data = self.data.lock().unwrap();
        if let Some(n) = data.notifications.remove(&id) {
            n.dismiss().unwrap_or_else(|err| log::error!("{}", err));
        }
    }

    fn get_capabilities(&self) -> Vec<&str> {
        log::debug!("get_capabilities");
        vec!["actions", "body"]
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

        match self.notify_internal(ctx, notification).await {
            Ok(id) => id,
            Err(err) => {
                log::error!("notify failed: {}", err);
                0
            }
        }
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
