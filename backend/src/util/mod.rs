pub mod vmcompute;
pub mod vmsocket;
pub mod wslpath;

#[macro_export]
macro_rules! unwrap_or_log {
    ( $e:expr ) => {
        match $e {
            Ok(x) => x,
            Err(e) => {
                log::error!("{}", e);
                return;
            }
        }
    };
}

pub fn log_err<T>(e: T) -> T
where
    T: std::error::Error,
{
    log::error!("{}", e);
    e
}
