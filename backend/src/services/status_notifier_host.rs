use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use futures::StreamExt;
use windows::Win32::{
    Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, PSTR, PWSTR, WPARAM},
    System::LibraryLoader::GetModuleHandleA,
    UI::WindowsAndMessaging::{
        CreateWindowExA, DispatchMessageA, GetMessageA, RegisterClassA, TranslateMessage,
        CW_USEDEFAULT, MSG, WINDOW_EX_STYLE, WNDCLASSA, WS_OVERLAPPEDWINDOW,
    },
};
use zbus::Connection;

use crate::{
    indicator::Indicator,
    proxies::status_notifier_watcher::{
        StatusNotifierItemRegisteredStream, StatusNotifierItemUnregisteredStream,
        StatusNotifierWatcherProxy,
    },
};

const WINDOW_CLASS_NAME: &[u8] = b"__hidden__\0";

struct HostInner {
    items: HashMap<String, Indicator>,
    connection: Connection,
}

#[derive(Clone)]
pub struct StatusNotifierHost {
    inner: Arc<Mutex<HostInner>>,
}

impl StatusNotifierHost {
    pub async fn init(connection: &Connection) -> zbus::Result<()> {
        let host = Self {
            inner: Arc::new(Mutex::new(HostInner {
                connection: connection.clone(),
                items: HashMap::new(),
            })),
        };

        let watcher_proxy = StatusNotifierWatcherProxy::new(connection).await?;

        {
            let host = host.clone();
            let item_registered_stream = watcher_proxy
                .receive_status_notifier_item_registered()
                .await?;

            tokio::spawn(async move {
                host.handle_item_registered(item_registered_stream)
                    .await
                    .unwrap_or_else(|e| log::error!("{}", e));
            });
        }

        {
            let host = host.clone();
            let item_unregistered_stream = watcher_proxy
                .receive_status_notifier_item_unregistered()
                .await?;

            tokio::spawn(async move {
                host.handle_item_unregistered(item_unregistered_stream)
                    .await
                    .unwrap_or_else(|e| log::error!("{}", e))
            });
        }

        Ok(())
    }

    fn create_window() -> windows::runtime::Result<HWND> {
        let instance = unsafe { GetModuleHandleA(None) };
        let mut window_class = WNDCLASSA::default();
        window_class.hInstance = instance;
        window_class.lpfnWndProc = Some(Self::wndproc);
        window_class.lpszClassName = PSTR(WINDOW_CLASS_NAME.as_ptr() as _);
        unsafe { RegisterClassA(&window_class) };

        let hwnd = unsafe {
            CreateWindowExA(
                WINDOW_EX_STYLE(0),
                PSTR(WINDOW_CLASS_NAME.as_ptr() as _),
                None,
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                None,
                None,
                instance,
                std::ptr::null(),
            )
        };

        std::thread::spawn(move || {
            let mut msg = MSG::default();
            unsafe {
                while GetMessageA(&mut msg, None, 0, 0).into() {
                    TranslateMessage(&msg);
                    DispatchMessageA(&msg);
                }
            };
        });

        Ok(hwnd)
    }

    unsafe extern "system" fn wndproc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
    }

    fn insert_item(&self, service: &str) -> anyhow::Result<bool> {
        let mut inner = self.inner.lock().unwrap();
        Ok(inner
            .items
            .insert(
                service.to_string(),
                Indicator::new(&inner.connection, service)?,
            )
            .is_none())
    }

    fn remove_item(&self, service: &str) -> bool {
        let mut inner = self.inner.lock().unwrap();
        inner.items.remove(service).is_some()
    }

    async fn handle_item_registered(
        self,
        mut stream: StatusNotifierItemRegisteredStream<'_>,
    ) -> anyhow::Result<()> {
        while let Some(signal) = stream.next().await {
            let args = signal.args()?;
            if let Err(e) = self.insert_item(args.service()) {
                log::error!("{}", e);
            }
        }

        Ok(())
    }

    async fn handle_item_unregistered(
        self,
        mut stream: StatusNotifierItemUnregisteredStream<'_>,
    ) -> anyhow::Result<()> {
        while let Some(signal) = stream.next().await {
            let args = signal.args()?;
            self.remove_item(args.service());
        }

        Ok(())
    }
}
