use rpc::notifications::{notifications_server::Notifications, NotifyRequest, NotifyResponse};
use tonic::{Request, Response, Status};

#[derive(Default)]
pub struct NotificationsService {}

#[tonic::async_trait]
impl Notifications for NotificationsService {
    async fn notify(
        &self,
        request: Request<NotifyRequest>,
    ) -> Result<Response<NotifyResponse>, Status> {
        println!("Got a request: {:?}", request);

        let reply = NotifyResponse {
            message: format!("Hello {}!", request.into_inner().name),
        };

        Ok(Response::new(reply))
    }
}
