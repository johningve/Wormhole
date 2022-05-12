// Copyright (c) 2022 John Ingve Olsen
//
// This software is released under the MIT License.
// https://opensource.org/licenses/MIT

use std::{
    borrow::Cow,
    convert::TryInto,
    sync::{Arc, Mutex},
    time::SystemTime,
};

use anyhow::bail;
use bimap::BiMap;
use lazy_static::lazy_static;
use regex::Regex;
use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    UI::WindowsAndMessaging::{
        AppendMenuW, CheckMenuItem, CheckMenuRadioItem, CreateMenu, CreatePopupMenu, DestroyMenu,
        GetMenu, PostMessageA, SetMenu, HMENU, MF_BYCOMMAND, MF_CHECKED, MF_DISABLED, MF_GRAYED,
        MF_POPUP, MF_SEPARATOR, MF_STRING, MF_UNCHECKED,
    },
};
use zvariant::Value;

use crate::{
    proxies::menu::{DBusMenuProxy, LayoutItem},
    services::status_notifier::host::WMAPP_SHOWMENU,
};

use super::host::MENU_IDS_PER_APP;

const PROPERTIES_USED: &[&str] = &[
    "type",
    "visible",
    "enabled",
    "label",
    // "icon-name",
    "toggle-type",
    "toggle-state",
];

struct MenuInner {
    next_id: u16,
    id_mapping: BiMap<u16, i32>,
    proxy: DBusMenuProxy<'static>,
}

#[derive(Clone)]
pub struct Menu(Arc<Mutex<MenuInner>>);

impl Menu {
    pub fn new(indicator_id: u16, proxy: DBusMenuProxy<'static>) -> anyhow::Result<Self> {
        let menu = Self(Arc::new(Mutex::new(MenuInner {
            next_id: indicator_id * MENU_IDS_PER_APP,
            id_mapping: BiMap::new(),
            proxy,
        })));

        Ok(menu)
    }

    fn get_proxy(&self) -> DBusMenuProxy {
        let inner = self.0.lock().unwrap();
        inner.proxy.clone()
    }

    fn map_id(&self, id: i32) -> u16 {
        let mut inner = self.0.lock().unwrap();

        if let Some(mapped_id) = inner.id_mapping.get_by_right(&id) {
            return *mapped_id;
        }

        let mapped_id = inner.next_id;
        inner.next_id += 1;
        inner.id_mapping.insert_no_overwrite(mapped_id, id).unwrap();

        if inner.next_id % MENU_IDS_PER_APP == 0 {
            panic!("ran out of menu IDs");
        }

        mapped_id
    }

    fn unmap_id(&self, mapped_id: u16) -> Option<i32> {
        let inner = self.0.lock().unwrap();

        inner.id_mapping.get_by_left(&mapped_id).copied()
    }

    fn add_item(&self, menu: &Win32Menu, item: &LayoutItem) -> anyhow::Result<()> {
        log::debug!("add_item");

        let item_type = match item.properties.get("type") {
            Some(v) => v.try_into()?,
            None => "",
        };

        if item_type == "separator" {
            menu.append_separator()?;
            return Ok(());
        }

        let label = match item.properties.get("label") {
            Some(v) => convert_access_key_hints(v.try_into()?),
            None => String::new(),
        };

        let enabled = match item.properties.get("enabled") {
            Some(v) => v.try_into()?,
            None => true,
        };

        if item.children.is_empty() {
            let id = self.map_id(item.id);
            menu.append_item(id, &label, enabled)?;

            let toggle_type = match item.properties.get("toggle-type") {
                Some(v) => v.try_into()?,
                None => "",
            };

            let toggle_state: i32 = match item.properties.get("toggle-state") {
                Some(v) => v.try_into()?,
                None => -1,
            };

            let checked = toggle_state == 1;

            if toggle_type == "checkmark" {
                if !menu.check_item(id, checked) {
                    bail!("failed to check menu item");
                }
            } else if toggle_type == "radio" {
                menu.check_radio_item(id, checked)?;
            }
        } else {
            let submenu = self.build_menu(item)?;
            menu.append_popup(&label, enabled, submenu)?;
        }

        Ok(())
    }

    fn build_menu(&self, layout: &LayoutItem) -> anyhow::Result<Win32Menu> {
        // TODO: figure out what to do about the root item.
        // for now, we'll just ignore it.
        log::debug!("build_menu");

        let menu = Win32Menu::create_popup()?;

        for ref child in layout.children()? {
            self.add_item(&menu, child)?;
        }

        Ok(menu)
    }

    pub async fn show_context_menu(&self, hwnd: HWND, x: i32, y: i32) -> anyhow::Result<()> {
        let (_, layout) = self.get_proxy().get_layout(0, -1, PROPERTIES_USED).await?;

        let menu = self.build_menu(&layout)?;

        log::debug!("menu was built");

        unsafe {
            PostMessageA(
                hwnd,
                WMAPP_SHOWMENU,
                WPARAM(menu.into_handle().0 as _),
                LPARAM(((x << 16) | y) as isize),
            )
        }
        .ok()?;

        Ok(())
    }

    pub(crate) async fn dispatch_command(&self, id: u16) -> anyhow::Result<()> {
        let unmapped_id = match self.unmap_id(id) {
            Some(id) => id,
            None => {
                log::warn!("no menu command with mapped id {}", id);

                return Ok(());
            }
        };

        log::debug!(
            "dispatching event for menu item (id: {} mapped: {})",
            unmapped_id,
            id
        );

        let proxy = {
            let inner = self.0.lock().unwrap();
            inner.proxy.clone()
        };

        proxy
            .event(
                unmapped_id,
                "clicked",
                &Value::new(""),
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)?
                    .as_secs() as _,
            )
            .await?;
        Ok(())
    }
}

pub struct Win32Menu(HMENU);

impl Drop for Win32Menu {
    fn drop(&mut self) {
        log::debug!("destroy_menu");

        unsafe { DestroyMenu(self.0) };
    }
}

impl Win32Menu {
    pub unsafe fn from_handle(handle: HMENU) -> Self {
        Self(handle)
    }

    fn get(hwnd: HWND) -> Option<Self> {
        let handle = unsafe { GetMenu(hwnd) };

        if handle.is_invalid() {
            None
        } else {
            Some(Self(handle))
        }
    }

    fn set(hwnd: HWND, hmenu: Option<Self>) -> windows::core::Result<()> {
        unsafe { SetMenu(hwnd, hmenu.map(Self::into_handle)) }.ok()
    }

    pub fn into_handle(self) -> HMENU {
        let handle = self.0;
        // don't drop this menu
        std::mem::forget(self);
        handle
    }

    pub unsafe fn handle(&self) -> HMENU {
        self.0
    }

    /// Creates a horizontal "menu bar" menu.
    fn create() -> windows::core::Result<Self> {
        log::debug!("create");

        unsafe { CreateMenu() }.ok().map(Self)
    }

    /// Creates a vertical "pop up" menu.
    fn create_popup() -> windows::core::Result<Self> {
        log::debug!("create_popup");

        unsafe { CreatePopupMenu() }.ok().map(Self)
    }

    fn append_item(&self, id: u16, label: &str, enabled: bool) -> windows::core::Result<()> {
        log::debug!("append_item");

        let mut flags = MF_STRING;

        if !enabled {
            flags |= MF_DISABLED | MF_GRAYED;
        }

        unsafe { AppendMenuW(self.0, flags, id as _, label) }.ok()
    }

    fn append_separator(&self) -> windows::core::Result<()> {
        log::debug!("append_separator");

        unsafe { AppendMenuW(self.0, MF_SEPARATOR, 0, None) }.ok()
    }

    fn append_popup(&self, label: &str, enabled: bool, popup: Self) -> windows::core::Result<()> {
        log::debug!("append_popup");

        let mut flags = MF_POPUP;

        if !enabled {
            flags |= MF_DISABLED | MF_GRAYED;
        }

        unsafe { AppendMenuW(self.0, flags, popup.into_handle().0 as _, label) }.ok()
    }

    #[allow(clippy::needless_return)]
    fn check_item(&self, id: u16, checked: bool) -> bool {
        log::debug!("check_item");

        return unsafe {
            CheckMenuItem(
                self.0,
                id as _,
                MF_BYCOMMAND | if checked { MF_CHECKED } else { MF_UNCHECKED },
            ) as i32
        } != -1;
    }

    // TODO: support item groups
    fn check_radio_item(&self, id: u16, checked: bool) -> windows::core::Result<()> {
        log::debug!("check_radio_item");

        if checked {
            unsafe { CheckMenuRadioItem(self.0, id as _, id as _, id as _, MF_BYCOMMAND) }.ok()
        } else {
            Ok(())
        }
    }
}

// TODO: make this faster
fn convert_access_key_hints(s: &str) -> String {
    lazy_static! {
        static ref ACCESS_KEY: Regex = Regex::new(r"_(\p{alpha})").unwrap();
        static ref LONELY_UNDERSCORE: Regex = Regex::new(r"_[^\p{alpha}_]").unwrap();
    }

    // first, replace all & with &&
    let s1: Cow<str> = s.replace("&", "&&").into();
    // then, replace single underscores
    let s2: Cow<str> = ACCESS_KEY.replace_all(&s1, "&$1");
    let s3 = LONELY_UNDERSCORE.replace_all(&s2, "");

    // then, replace double underscores
    s3.replace("__", "_")
}
