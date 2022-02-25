use windows::Win32::Foundation::{ERROR_SUCCESS, WIN32_ERROR};

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

#[macro_export]
macro_rules! hiword {
    ($e:expr) => {
        $e >> 16
    };
}

#[macro_export]
macro_rules! loword {
    ($e:expr) => {
        $e & 0xffff
    };
}

#[allow(dead_code)]
pub fn log_err<T>(e: T) -> T
where
    T: std::error::Error,
{
    log::error!("{}", e);
    e
}

pub fn as_win32_result(err: WIN32_ERROR) -> windows::core::Result<()> {
    match err {
        ERROR_SUCCESS => Ok(()),
        _ => Err(windows::core::Error::fast_error(
            windows::core::HRESULT::from_win32(err),
        )),
    }
}
