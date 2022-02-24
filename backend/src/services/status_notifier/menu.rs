use std::{
    collections::HashMap,
    convert::{TryFrom, TryInto},
    slice::SliceIndex,
    sync::{Arc, Mutex},
};

use anyhow::bail;
use bimap::BiMap;
use tokio::sync::oneshot;
use windows::Win32::{
    Foundation::{HWND, POINT},
    UI::WindowsAndMessaging::{
        AppendMenuW, CheckMenuItem, CheckMenuRadioItem, CreateMenu, CreatePopupMenu, DestroyMenu,
        GetMenu, SetMenu, HMENU, MFT_RADIOCHECK, MF_BYCOMMAND, MF_CHECKED, MF_DISABLED, MF_GRAYED,
        MF_POPUP, MF_SEPARATOR, MF_STRING, MF_UNCHECKED,
    },
};
use zvariant::OwnedValue;

use crate::proxies::menu::{DBusMenuProxy, LayoutItem};

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

        if let Some(id) = inner.id_mapping.get_by_left(&mapped_id) {
            Some(*id)
        } else {
            None
        }
    }

    fn add_item(&self, menu: &Win32Menu, item: &LayoutItem) -> anyhow::Result<()> {
        let item_type = match item.properties.get("type") {
            Some(v) => v.try_into()?,
            None => "",
        };

        if item_type == "separator" {
            menu.append_separator()?;
            return Ok(());
        }

        let label = match item.properties.get("label") {
            Some(v) => v.try_into()?,
            None => "",
        };

        let enabled = match item.properties.get("enabled") {
            Some(v) => v.try_into()?,
            None => true,
        };

        if item.children.is_empty() {
            let id = self.map_id(item.id);
            menu.append_item(id, label, enabled)?;

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
                    bail!("failed to checck menu item");
                }
            } else if toggle_type == "radio" {
                menu.check_radio_item(id, checked)?;
            }
        } else {
            let submenu = self.build_menu(&item)?;
            menu.append_popup(label, enabled, submenu)?;
        }

        Ok(())
    }

    fn build_menu(&self, layout: &LayoutItem) -> anyhow::Result<Win32Menu> {
        // TODO: figure out what to do about the root item.
        // for now, we'll just ignore it.

        let menu = Win32Menu::create_popup()?;

        for ref child in layout.children()? {
            self.add_item(&menu, child)?;
        }

        return Ok(menu);
    }

    pub async fn show_context_menu(&self, hwnd: HWND, point: POINT) -> anyhow::Result<()> {
        let (_, layout) = self.get_proxy().get_layout(0, -1, PROPERTIES_USED).await?;

        let menu = self.build_menu(&layout)?;

        // TODO: show the menu :^)

        Ok(())
    }
}

// impl Drop for Menu {
//     fn drop(&mut self) {
//         unsafe { DestroyMenu(self.handle) };
//     }
// }

// impl Menu {
//     fn new() -> windows::core::Result<Self> {
//         let handle = unsafe { CreateMenu() };
//         if handle.is_invalid() {
//             return Err(windows::core::Error::from_win32());
//         }

//         Ok(Self { handle })
//     }

//     fn append_item(&self, id: usize, label: &str) -> windows::core::Result<()> {
//         if !unsafe { AppendMenuW(self.handle, MF_STRING, id, label) }.as_bool() {
//             return Err(windows::core::Error::from_win32());
//         }
//         Ok(())
//     }
// }

struct Win32Menu(HMENU);

impl Drop for Win32Menu {
    fn drop(&mut self) {
        unsafe { DestroyMenu(self.0) };
    }
}

impl Win32Menu {
    fn get(hwnd: HWND) -> Option<Self> {
        let handle = unsafe { GetMenu(hwnd) };

        if handle.is_invalid() {
            None
        } else {
            Some(Self(handle))
        }
    }

    fn set(hwnd: HWND, hmenu: Option<Self>) -> windows::core::Result<()> {
        if unsafe { SetMenu(hwnd, hmenu.map(Self::into_handle)) }.as_bool() {
            Ok(())
        } else {
            Err(windows::core::Error::from_win32())
        }
    }

    fn into_handle(self) -> HMENU {
        let handle = self.0;
        // don't drop this menu
        std::mem::forget(self);
        handle
    }

    unsafe fn handle(&self) -> HMENU {
        self.0
    }

    fn create() -> windows::core::Result<Self> {
        let handle = unsafe { CreateMenu() };
        if handle.is_invalid() {
            Err(windows::core::Error::from_win32())
        } else {
            Ok(Self(handle))
        }
    }

    fn create_popup() -> windows::core::Result<Self> {
        let handle = unsafe { CreatePopupMenu() };
        if handle.is_invalid() {
            Err(windows::core::Error::from_win32())
        } else {
            Ok(Self(handle))
        }
    }

    fn append_item(&self, id: u16, label: &str, enabled: bool) -> windows::core::Result<()> {
        let mut flags = MF_STRING;

        if !enabled {
            flags |= MF_DISABLED | MF_GRAYED;
        }

        if unsafe { AppendMenuW(self.0, flags, id as _, label) }.as_bool() {
            Ok(())
        } else {
            Err(windows::core::Error::from_win32())
        }
    }

    fn append_separator(&self) -> windows::core::Result<()> {
        if unsafe { AppendMenuW(self.0, MF_SEPARATOR, 0, None) }.as_bool() {
            Ok(())
        } else {
            Err(windows::core::Error::from_win32())
        }
    }

    fn append_popup(&self, label: &str, enabled: bool, popup: Self) -> windows::core::Result<()> {
        let mut flags = MF_POPUP;

        if !enabled {
            flags |= MF_DISABLED | MF_GRAYED;
        }

        if unsafe { AppendMenuW(self.0, flags, popup.into_handle().0 as _, None) }.as_bool() {
            Ok(())
        } else {
            Err(windows::core::Error::from_win32())
        }
    }

    #[allow(clippy::needless_return)]
    fn check_item(&self, id: u16, checked: bool) -> bool {
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
        if unsafe { CheckMenuRadioItem(self.0, id as _, id as _, id as _, MF_BYCOMMAND) }.as_bool()
        {
            Ok(())
        } else {
            Err(windows::core::Error::from_win32())
        }
    }
}

enum ToggleType {
    None,
    Checkbox,
    Radiobutton,
}
