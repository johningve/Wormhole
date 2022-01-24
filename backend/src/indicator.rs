use std::sync::{Arc, Mutex};

use scopeguard::defer;
use windows::Win32::{
    Foundation::HWND,
    Graphics::Gdi::{CreateBitmap, CreateCompatibleBitmap, DeleteObject, HDC},
    UI::{
        Shell::{
            Shell_NotifyIconA, NIF_ICON, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
            NIM_SETVERSION, NOTIFYICONDATAA, NOTIFYICON_VERSION_4,
        },
        WindowsAndMessaging::{CreateIconIndirect, DestroyIcon, HICON, ICONINFO},
    },
};
use zbus::Connection;

use crate::proxies::status_notifier_item::StatusNotifierItemProxy;

pub struct IndicatorInner {
    id: u32,
    hwnd: HWND,
    icon: Icon,
    tooltip: String,
    shown: bool,
}

pub struct Indicator {
    inner: Arc<Mutex<IndicatorInner>>,
}

impl Indicator {
    pub fn new(
        hwnd: HWND,
        connection: &Connection,
        id: u32,
        service: &str,
    ) -> anyhow::Result<Self> {
        let mut indicator = Indicator {
            inner: Arc::new(Mutex::new(IndicatorInner {
                id,
                hwnd,
                icon: Icon::default(),
                tooltip: String::from(""),
                shown: false,
            })),
        };

        tokio::spawn(indicator.init(&connection.clone(), &service.to_owned()));

        Ok(indicator)
    }

    pub async fn init(&self, connection: &Connection, service: &str) {
        let item_proxy = StatusNotifierItemProxy::builder(connection)
            .destination(service)?
            .build()
            .await;
    }

    fn update(&self, icon: Icon, tooltip: &str) {
        let mut inner = self.inner.lock().unwrap();

        let mut data = NOTIFYICONDATAA::default();
        data.cbSize = std::mem::size_of::<NOTIFYICONDATAA>() as _;
        data.uFlags = NIF_ICON | NIF_TIP | NIF_SHOWTIP;
        data.Anonymous.uVersion = NOTIFYICON_VERSION_4;

        data.uID = inner.id;
        data.hWnd = inner.hwnd;
        data.hIcon = icon.0;
        for (i, c) in data.szInfo.iter().enumerate() {
            if i < tooltip.len() {
                c.0 = tooltip.as_bytes()[i];
            }
        }

        let success = if !inner.shown {
            unsafe { Shell_NotifyIconA(NIM_ADD, &data) }.as_bool()
                && unsafe { Shell_NotifyIconA(NIM_SETVERSION, &data) }.as_bool()
        } else {
            unsafe { Shell_NotifyIconA(NIM_MODIFY, &data) }.as_bool()
        };

        if !success {
            log::error!("Failed to show icon!");
        }

        inner.tooltip = tooltip.to_string();
        inner.icon = icon;
        inner.shown = true;
    }
}

impl Drop for Indicator {
    fn drop(&mut self) {
        let mut inner = self.inner.try_lock().unwrap();

        let mut data = NOTIFYICONDATAA::default();
        data.cbSize = std::mem::size_of::<NOTIFYICONDATAA>() as _;
        data.uFlags = NIF_ICON | NIF_TIP | NIF_SHOWTIP;
        data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
        data.uID = inner.id;
        data.hWnd = inner.hwnd;

        unsafe { Shell_NotifyIconA(NIM_DELETE, &data) };
    }
}

#[derive(Default)]
struct Icon(HICON);

impl Drop for Icon {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe { DestroyIcon(self.0) };
        }
    }
}

impl Icon {
    fn from_argb(
        dc: HDC,
        width: u32,
        height: u32,
        argb_bytes: &[u8],
    ) -> windows::core::Result<Self> {
        // based on https://stackoverflow.com/a/62614596/16076168
        let icon_info = ICONINFO::default();

        let mut raw_bitmap = vec![0u32; argb_bytes.len() / 4];

        for y in 0..height {
            for x in 0..width {
                let index = (x + y * width) as usize;
                let base = index * 4;
                let a = argb_bytes[base] as u32;
                let r = argb_bytes[base + 1] as u32;
                let g = argb_bytes[base + 2] as u32;
                let b = argb_bytes[base + 3] as u32;
                raw_bitmap[index] = (a << 24) | (r << 16) | (g << 8) | b; // the result should be BRGA
            }
        }

        icon_info.hbmColor =
            unsafe { CreateBitmap(width as _, height as _, 1, 32, raw_bitmap.as_ptr() as _) };
        if icon_info.hbmColor.is_invalid() {
            return Err(windows::core::Error::from_win32());
        }
        defer! {unsafe {DeleteObject(icon_info.hbmColor)};}

        icon_info.hbmMask = unsafe { CreateCompatibleBitmap(dc, width as _, height as _) };
        if icon_info.hbmMask.is_invalid() {
            return Err(windows::core::Error::from_win32());
        }
        defer! {unsafe {DeleteObject(icon_info.hbmMask)};}

        let icon = unsafe { CreateIconIndirect(&icon_info) };
        if icon.is_invalid() {
            return Err(windows::core::Error::from_win32());
        }

        Ok(Icon(icon))
    }
}
