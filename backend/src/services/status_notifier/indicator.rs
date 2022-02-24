use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use futures::StreamExt;
use scopeguard::defer;
use tokio::sync::oneshot;
use windows::Win32::{
    Foundation::HWND,
    Graphics::Gdi::{GetDC, ReleaseDC},
};

use crate::{
    proxies::{
        icons::IconsProxy,
        status_notifier_item::{Pixmap, StatusNotifierItemProxy},
    },
    util::wslpath,
};

use super::{icon::Icon, systray::SysTrayIcon};

struct IndicatorInner {
    icon: SysTrayIcon,
    proxy: StatusNotifierItemProxy<'static>,
    close: Option<oneshot::Sender<()>>,
}

#[derive(Clone)]
pub struct Indicator(Arc<Mutex<IndicatorInner>>);

impl Indicator {
    pub fn new(
        hwnd: HWND,
        id: u16,
        proxy: StatusNotifierItemProxy<'static>,
    ) -> anyhow::Result<Self> {
        let (tx, rx) = oneshot::channel();

        let indicator = Self(Arc::new(Mutex::new(IndicatorInner {
            icon: SysTrayIcon::new(hwnd, id),
            proxy,
            close: Some(tx),
        })));

        {
            let indicator = indicator.clone();
            tokio::spawn(async move { indicator.handle_updates(rx).await });
        }

        {
            let indicator = indicator.clone();
            tokio::spawn(async move { indicator.update().await.unwrap() });
        }

        Ok(indicator)
    }

    // TODO: figure out how to handle errors from this task
    async fn handle_updates(&self, mut close: oneshot::Receiver<()>) {
        let proxy = { self.0.lock().unwrap().proxy.clone() };

        let mut new_status_stream = proxy.receive_new_status().await.unwrap();
        let mut new_icon_stream = proxy.receive_new_icon().await.unwrap();
        let mut new_attention_icon_stream = proxy.receive_new_attention_icon().await.unwrap();
        let mut new_tooltip_stream = proxy.receive_new_tooltip().await.unwrap();

        // don't care about the contents of these signals, as none of them carry any arguments.
        while tokio::select! {
            s = new_status_stream.next() => s.is_some(),
            s = new_icon_stream.next() => s.is_some(),
            s = new_attention_icon_stream.next() => s.is_some(),
            s = new_tooltip_stream.next() => s.is_some(),
            _ = &mut close => false,
        } {
            self.update().await.unwrap();
        }
    }

    async fn update(&self) -> anyhow::Result<()> {
        log::debug!("update");

        let (proxy, hwnd) = {
            let inner = self.0.lock().unwrap();
            (inner.proxy.clone(), inner.icon.hwnd)
        };

        // FIXME: might want to be more careful unwrapping these.
        // Errors could be useful.
        let status = proxy.status().await.unwrap_or_default();
        let tooltip = proxy.tool_tip().await.unwrap_or_default();
        let icon_name = if status == "NeedsAttention" {
            proxy.attention_icon_name().await.unwrap_or_default()
        } else {
            proxy.icon_name().await.unwrap_or_default()
        };
        let icon_pixmap = if status == "NeedsAttention" {
            proxy.attention_icon_pixmap().await.unwrap_or_default()
        } else {
            proxy.icon_pixmap().await.unwrap_or_default()
        };

        let icons_proxy = IconsProxy::new(proxy.connection()).await?;
        let icon_path = wslpath::to_windows(&icons_proxy.lookup_icon(&icon_name, 32).await?);

        let icon = get_icon(hwnd, &icon_path, icon_pixmap)?;

        let tooltip_text = format!("{}: {}", tooltip.title, tooltip.description);

        // TODO: might consider not updating icon if it has not changed
        self.0
            .lock()
            .unwrap()
            .icon
            .update(Some(icon), Some(&tooltip_text));

        Ok(())
    }

    pub fn unregister(self) {
        let mut inner = self.0.lock().unwrap();

        log::debug!("unregister");

        if let Some(tx) = inner.close.take() {
            let _ = tx.send(());
        }
    }

    pub fn id(&self) -> u16 {
        let inner = self.0.lock().unwrap();
        inner.icon.id
    }

    pub async fn activate(&self, x: i32, y: i32) -> zbus::Result<()> {
        log::debug!("activate: ({}, {})", x, y);
        let proxy = {
            let inner = self.0.lock().unwrap();
            inner.proxy.clone()
        };
        proxy.activate(x, y).await
    }

    pub async fn secondary_activate(&self, x: i32, y: i32) -> zbus::Result<()> {
        log::debug!("secondary_activate: ({}, {})", x, y);
        let proxy = {
            let inner = self.0.lock().unwrap();
            inner.proxy.clone()
        };

        if proxy.item_is_menu().await.unwrap_or_default() {
            proxy.context_menu(x, y).await
        } else {
            proxy.secondary_activate(x, y).await
        }
    }
}

fn get_icon(hwnd: HWND, icon_path: &Path, icon_pixmaps: Vec<Pixmap>) -> anyhow::Result<Icon> {
    let dc = unsafe { GetDC(hwnd) };
    if dc.is_invalid() {
        return Err(windows::core::Error::from_win32().into());
    }
    defer! { unsafe { ReleaseDC(hwnd, dc) }; }

    let icon = if icon_pixmaps.is_empty() {
        let image = image::open(icon_path)?.into_bgra8();
        Icon::from_bgra(dc, image.width(), image.height(), &image)?
    } else {
        // TODO: smarter selection
        let pixmap = icon_pixmaps.get(0).unwrap();
        Icon::from_argb32_network_order(
            dc,
            pixmap.width as _,
            pixmap.height as _,
            &pixmap.image_data,
        )?
    };

    Ok(icon)
}
