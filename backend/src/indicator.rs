use std::sync::{Arc, Mutex};

use scopeguard::defer;
use windows::Win32::{
    Foundation::HWND,
    Graphics::Gdi::{CreateBitmap, CreateCompatibleBitmap, DeleteObject, HDC},
    UI::WindowsAndMessaging::{CreateIconIndirect, DestroyIcon, HICON, ICONINFO},
};
use zbus::Connection;

use crate::proxies::status_notifier_item::{Pixmap, StatusNotifierItemProxy};

pub struct IndicatorInner<'a> {
    hwnd: HWND,
    proxy: Option<StatusNotifierItemProxy<'a>>,
}

pub struct Indicator<'a> {
    inner: Arc<Mutex<IndicatorInner<'a>>>,
}

impl Indicator<'_> {
    pub fn new(hwnd: HWND, connection: &Connection, service: &str) -> anyhow::Result<Self> {
        let mut indicator = Indicator {
            inner: Arc::new(Mutex::new(IndicatorInner { hwnd, proxy: None })),
        };

        tokio::spawn(indicator.init(&connection.clone(), &service.to_owned()));

        Ok(indicator)
    }

    pub async fn init(&self, connection: &Connection, service: &str) {
        let item_proxy = StatusNotifierItemProxy::builder(connection)
            .destination(service)?
            .build()
            .await;

        let mut inner = self.inner.lock().unwrap();
        inner.proxy = Some(item_proxy);

        inner.proxy.unwrap().
    }
}

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
