// Copyright (c) 2022 John Ingve Olsen
//
// This software is released under the MIT License.
// https://opensource.org/licenses/MIT

use zbus::Connection;

use self::{
    filechooser::FileChooser, notifications::Notifications,
    status_notifier::watcher::StatusNotifierWatcher,
};

pub mod filechooser;
pub mod notifications;
pub mod status_notifier;

pub const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";

pub async fn init_all(connection: &Connection) -> anyhow::Result<()> {
    FileChooser::init(connection).await?;
    Notifications::init(connection).await?;
    StatusNotifierWatcher::init(connection).await?;

    Ok(())
}
