use std::error::Error;

use tonic::{Request, Status};

pub fn map_to_status(e: impl Into<Box<dyn Error>>) -> Status {
    Status::new(tonic::Code::Internal, e.into().to_string())
}

pub fn get_distro_name<T>(request: &Request<T>) -> Result<String, Status> {
    match request.metadata().get("wsl-distro-name") {
        Some(name) => name.to_str().map(str::to_string).map_err(map_to_status),
        None => Err(tonic::Status::new(
            tonic::Code::Internal,
            "missing distro name",
        )),
    }
}
