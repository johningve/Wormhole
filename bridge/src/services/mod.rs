use zbus::Connection;

pub mod icons;

pub async fn init_all(connection: &Connection) -> zbus::Result<()> {
    icons::Icons::init(connection).await?;

    Ok(())
}
