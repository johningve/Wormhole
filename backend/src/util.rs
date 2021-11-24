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
