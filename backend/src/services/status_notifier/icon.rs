use scopeguard::defer;
use windows::Win32::{
    Graphics::Gdi::{CreateBitmap, CreateCompatibleBitmap, DeleteObject, HDC},
    UI::WindowsAndMessaging::{CreateIconIndirect, DestroyIcon, HICON, ICONINFO},
};

#[derive(Default)]
pub struct Icon(HICON);

impl Drop for Icon {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe { DestroyIcon(self.0) };
        }
    }
}

impl Icon {
    pub fn handle(&self) -> HICON {
        self.0
    }

    // TODO: I think the network order ARGB32 might be equivalent to what GDI wants...
    pub fn from_argb32_network_order(
        dc: HDC,
        width: u32,
        height: u32,
        argb_bytes: &[u8],
    ) -> windows::core::Result<Self> {
        // based on https://stackoverflow.com/a/62614596/16076168
        let mut bgra_bytes = vec![0u8; argb_bytes.len()];

        for y in 0..height {
            for x in 0..width {
                let index = (x + y * width) as usize;
                let base = index * 4;
                bgra_bytes[base + 3] = argb_bytes[base]; // a
                bgra_bytes[base + 2] = argb_bytes[base + 1]; // r
                bgra_bytes[base + 1] = argb_bytes[base + 2]; // g
                bgra_bytes[base] = argb_bytes[base + 3]; // b
            }
        }

        Self::from_bgra(dc, width, height, &bgra_bytes)
    }

    pub fn from_bgra(
        dc: HDC,
        width: u32,
        height: u32,
        bgra_bytes: &[u8],
    ) -> windows::core::Result<Self> {
        let icon_info = ICONINFO::default();

        icon_info.hbmColor =
            unsafe { CreateBitmap(width as _, height as _, 1, 32, bgra_bytes.as_ptr() as _) };
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
