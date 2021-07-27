use std::{convert::TryFrom, error::Error};

use zbus::{fdo, Connection, ObjectServer};
use zvariant::ObjectPath;

use notifications::Notifications;

mod notifications;

fn main() -> Result<(), Box<dyn Error>> {
    let connection = Connection::new_session()?;

    fdo::DBusProxy::new(&connection)?.request_name(
        "org.freedesktop.Notifications",
        fdo::RequestNameFlags::ReplaceExisting.into(),
    )?;

    let mut object_server = ObjectServer::new(&connection);
    let notifications = Notifications {};
    object_server.at(
        &ObjectPath::try_from("/org/freedesktop/Notifications")?,
        notifications,
    )?;

    loop {
        if let Err(err) = object_server.try_handle_next() {
            eprintln!("{}", err);
        }
    }
}
