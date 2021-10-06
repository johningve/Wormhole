use rpc::notifications::{
    notifications_server::Notifications, notify_response::Event, CloseNotificationRequest,
    NotificationActionInvoked, NotificationCreated, NotificationDismissed, NotifyRequest,
    NotifyResponse,
};
use std::{collections::BTreeMap, sync::Mutex};
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tonic::codegen::http::request;

use crate::toasthelper::ToastHelper;

struct Notification {
    toast: ToastHelper,
}

#[derive(Default)]
struct NotificationsServiceData {
    next_id: u32,
    notifications: BTreeMap<u32, Notification>,
}

#[derive(Default)]
pub struct NotificationsService {
    data: Mutex<NotificationsServiceData>,
}

#[tonic::async_trait]
impl Notifications for NotificationsService {
    type NotifyStream = UnboundedReceiverStream<Result<NotifyResponse, tonic::Status>>;

    async fn notify(
        &self,
        request: tonic::Request<NotifyRequest>,
    ) -> Result<tonic::Response<Self::NotifyStream>, tonic::Status> {
        let mut data = self.data.lock().unwrap();
        let id = data.next_id;
        data.next_id += 1;

        let err_to_status = |err: windows::Error| -> tonic::Status {
            tonic::Status::new(tonic::Code::Internal, err.to_string())
        };

        let distro_name = match request.metadata().get("wsl-distro-name") {
            Some(name) => name
                .to_str()
                .map_err(|err| tonic::Status::new(tonic::Code::Internal, err.to_string()))?,
            None => {
                return Err(tonic::Status::new(
                    tonic::Code::Internal,
                    "missing distro name",
                ));
            }
        };

        let request = request.get_ref();
        let toast =
            ToastHelper::from(request, &id.to_string(), distro_name).map_err(err_to_status)?;

        let (tx, rx) = mpsc::unbounded_channel();

        {
            let tx = tx.clone();
            toast
                .on_activated(move |action| {
                    tx.send(Ok(NotifyResponse {
                        event: Some(Event::ActionInvoked(NotificationActionInvoked {
                            id,
                            action,
                        })),
                    }))
                    .unwrap_or_else(|err| log::error!("{}", err));
                })
                .map_err(err_to_status)?;
        }

        {
            let tx = tx.clone();
            toast
                .on_dismissed(move || {
                    tx.send(Ok(NotifyResponse {
                        event: Some(Event::Dismissed(NotificationDismissed { id })),
                    }))
                    .unwrap_or_else(|err| log::error!("{}", err));
                })
                .map_err(err_to_status)?;
        }

        {
            let tx = tx.clone();
            toast
                .on_failed(move |err| {
                    tx.send(Err(err_to_status(err)))
                        .unwrap_or_else(|err| log::error!("{}", err));
                })
                .map_err(err_to_status)?;
        }

        tx.send(Ok(NotifyResponse {
            event: Some(Event::Created(NotificationCreated { id })),
        }))
        .unwrap_or_else(|err| log::error!("{}", err));

        toast.show().map_err(err_to_status)?;
        data.notifications.insert(id, Notification { toast });

        Ok(tonic::Response::new(UnboundedReceiverStream::new(rx)))
    }

    async fn close_notification(
        &self,
        request: tonic::Request<CloseNotificationRequest>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        let request = request.into_inner();
        let mut data = self.data.lock().unwrap();
        if let Some(n) = data.notifications.remove(&request.id) {
            n.toast
                .dismiss()
                .unwrap_or_else(|err| log::error!("{}", err));
        }
        Ok(tonic::Response::new(()))
    }
}
