use zbus::Connection;

use crate::proxies::status_notifier_item::StatusNotifierItemProxy;

pub struct Indicator {}

impl Indicator {
    pub async fn new(connection: &Connection, service: &str) -> anyhow::Result<Self> {
        let item_proxy = StatusNotifierItemProxy::builder(connection)
            .destination(service)?
            .build()
            .await;

        Ok(Indicator {})
    }
}
