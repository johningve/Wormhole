// Copyright (c) 2022 John Ingve Olsen
//
// This software is released under the MIT License.
// https://opensource.org/licenses/MIT

use zbus::dbus_proxy;

#[dbus_proxy(
    interface = "com.github.raytar.Icons",
    default_service = "com.github.raytar.Icons",
    default_path = "/com/github/raytar/Icons"
)]
pub trait Icons {
    fn lookup_icon(&self, icon: &str, size: u16) -> zbus::Result<String>;
}
