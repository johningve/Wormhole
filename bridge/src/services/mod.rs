use zbus::Connection;

pub mod icons;
pub mod wsl;

pub async fn init_all(connection: &Connection) -> zbus::Result<()> {
    icons::Icons::init(connection).await?;
    wsl::WSL::init(connection).await?;

    Ok(())
}
