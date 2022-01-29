use windows::Win32::{
    Foundation::HWND,
    UI::Shell::{
        Shell_NotifyIconA, NIF_ICON, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
        NIM_SETVERSION, NOTIFYICONDATAA, NOTIFYICONDATAA_0, NOTIFYICON_VERSION_4,
    },
};

use super::icon::Icon;

/// Indicator is responsible for displaying an application indicator in the notification area.
pub struct SysTrayIcon {
    pub id: u32,
    pub hwnd: HWND,
    icon: Icon,
    tooltip: String,
    shown: bool,
}

impl SysTrayIcon {
    pub fn new(hwnd: HWND, id: u32) -> Self {
        SysTrayIcon {
            id,
            hwnd,
            icon: Icon::default(),
            tooltip: String::from(""),
            shown: false,
        }
    }

    pub fn update(&mut self, icon: Option<Icon>, tooltip: Option<&str>) {
        log::debug!("update");

        let mut data = NOTIFYICONDATAA {
            cbSize: std::mem::size_of::<NOTIFYICONDATAA>() as _,
            uFlags: NIF_ICON | NIF_TIP | NIF_SHOWTIP,
            Anonymous: NOTIFYICONDATAA_0 {
                uVersion: NOTIFYICON_VERSION_4,
            },
            uID: self.id,
            hWnd: self.hwnd,
            ..Default::default()
        };

        data.hIcon = if icon.is_some() {
            icon.as_ref().unwrap().handle()
        } else {
            self.icon.handle()
        };

        if let Some(tooltip) = tooltip {
            self.tooltip = tooltip.to_string();
        }

        for (i, c) in data.szInfo.iter_mut().enumerate() {
            if i < self.tooltip.len() {
                c.0 = self.tooltip.as_bytes()[i];
            }
        }

        let success = if !self.shown {
            unsafe { Shell_NotifyIconA(NIM_ADD, &data) }.as_bool()
                && unsafe { Shell_NotifyIconA(NIM_SETVERSION, &data) }.as_bool()
        } else {
            unsafe { Shell_NotifyIconA(NIM_MODIFY, &data) }.as_bool()
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

        let data = NOTIFYICONDATAA {
            cbSize: std::mem::size_of::<NOTIFYICONDATAA>() as _,
            Anonymous: NOTIFYICONDATAA_0 {
                uVersion: NOTIFYICON_VERSION_4,
            },
            uID: self.id,
            hWnd: self.hwnd,
            ..Default::default()
        };

        unsafe { Shell_NotifyIconA(NIM_DELETE, &data) };
    }
}
