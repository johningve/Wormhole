use windows::Win32::{
    Foundation::HWND,
    UI::Shell::{
        Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE,
        NIM_MODIFY, NIM_SETVERSION, NOTIFYICONDATAW, NOTIFYICONDATAW_0, NOTIFYICON_VERSION_4,
    },
};

use crate::services::status_notifier::host::WMAPP_NOTIFYCALLBACK;

use super::icon::Icon;

/// Indicator is responsible for displaying an application indicator in the notification area.
pub struct SysTrayIcon {
    pub id: u16,
    pub hwnd: HWND,
    icon: Icon,
    tooltip: Vec<u16>,
    shown: bool,
}

impl SysTrayIcon {
    pub fn new(hwnd: HWND, id: u16) -> Self {
        SysTrayIcon {
            id,
            hwnd,
            icon: Icon::default(),
            tooltip: vec![0u16],
            shown: false,
        }
    }

    pub fn update(&mut self, icon: Option<Icon>, tooltip: Option<&str>) {
        log::debug!("update");

        let mut data = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as _,
            uFlags: NIF_ICON | NIF_TIP | NIF_SHOWTIP | NIF_MESSAGE,
            uCallbackMessage: WMAPP_NOTIFYCALLBACK,
            Anonymous: NOTIFYICONDATAW_0 {
                uVersion: NOTIFYICON_VERSION_4,
            },
            uID: self.id as _,
            hWnd: self.hwnd,
            ..Default::default()
        };

        data.hIcon = if icon.is_some() {
            icon.as_ref().unwrap().handle()
        } else {
            self.icon.handle()
        };

        if let Some(tooltip) = tooltip {
            self.tooltip = tooltip.encode_utf16().chain([0u16]).collect();
        }

        for (i, c) in data.szTip.iter_mut().enumerate() {
            if i < self.tooltip.len() {
                *c = self.tooltip[i];
            } else {
                break;
            }
        }

        let success = if !self.shown {
            unsafe { Shell_NotifyIconW(NIM_ADD, &data) }.as_bool()
                && unsafe { Shell_NotifyIconW(NIM_SETVERSION, &data) }.as_bool()
        } else {
            unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) }.as_bool()
        };

        if !success {
            log::error!("Failed to show icon!");
        }

        self.shown = true;

        // must swap icon late so that the old icon is dropped last
        if let Some(icon) = icon {
            self.icon = icon;
        }
    }
}

impl Drop for SysTrayIcon {
    fn drop(&mut self) {
        log::debug!("drop");

        let data = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as _,
            uCallbackMessage: WMAPP_NOTIFYCALLBACK,
            Anonymous: NOTIFYICONDATAW_0 {
                uVersion: NOTIFYICON_VERSION_4,
            },
            uID: self.id as _,
            hWnd: self.hwnd,
            ..Default::default()
        };

        unsafe { Shell_NotifyIconW(NIM_DELETE, &data) };
    }
}
