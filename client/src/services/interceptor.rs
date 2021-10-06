use tonic::service::Interceptor;

#[derive(Clone)]
pub struct DistroInterceptor {}

impl Interceptor for DistroInterceptor {
    fn call(
        &mut self,
        mut request: tonic::Request<()>,
    ) -> Result<tonic::Request<()>, tonic::Status> {
        request.metadata_mut().insert(
            "wsl-distro-name",
            std::env::var("WSL_DISTRO_NAME")
                .unwrap_or_default()
                .parse()
                .unwrap(),
        );

        Ok(request)
    }
}
