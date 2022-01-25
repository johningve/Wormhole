use windows::Win32::{
    Foundation::HWND,
    UI::Shell::{
        Shell_NotifyIconA, NIF_ICON, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
        NIM_SETVERSION, NOTIFYICONDATAA, NOTIFYICON_VERSION_4,
    },
};

use super::icon::Icon;

/// Indicator is responsible for displaying an application indicator in the notification area.
pub struct Indicator {
    id: u32,
    hwnd: HWND,
    icon: Icon,
    tooltip: String,
    shown: bool,
}

impl Indicator {
    pub fn new(hwnd: HWND, id: u32) -> Self {
        Indicator {
            id,
            hwnd,
            icon: Icon::default(),
            tooltip: String::from(""),
            shown: false,
        }
    }

    fn update(&self, icon: Icon, tooltip: &str) {
        let mut data = NOTIFYICONDATAA::default();
        data.cbSize = std::mem::size_of::<NOTIFYICONDATAA>() as _;
        data.uFlags = NIF_ICON | NIF_TIP | NIF_SHOWTIP;
        data.Anonymous.uVersion = NOTIFYICON_VERSION_4;

        data.uID = self.id;
        data.hWnd = self.hwnd;
        data.hIcon = icon.handle();
        for (i, c) in data.szInfo.iter().enumerate() {
            if i < tooltip.len() {
                c.0 = tooltip.as_bytes()[i];
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

        self.tooltip = tooltip.to_string();
        self.icon = icon;
        self.shown = true;
    }
}

impl Drop for Indicator {
    fn drop(&mut self) {
        let mut data = NOTIFYICONDATAA::default();
        data.cbSize = std::mem::size_of::<NOTIFYICONDATAA>() as _;
        data.uFlags = NIF_ICON | NIF_TIP | NIF_SHOWTIP;
        data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
        data.uID = self.id;
        data.hWnd = self.hwnd;

        unsafe { Shell_NotifyIconA(NIM_DELETE, &data) };
    }
}
