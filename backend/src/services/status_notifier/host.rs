use std::{
    cell::RefCell,
    collections::HashMap,
    convert::TryFrom,
    sync::{mpsc, Arc, Mutex},
};

use anyhow::bail;
use futures::StreamExt;
use windows::Win32::{
    Foundation::{BOOL, HWND, LPARAM, LRESULT, PSTR, WPARAM},
    System::LibraryLoader::GetModuleHandleA,
    UI::{
        Controls::RichEdit::WM_CONTEXTMENU,
        WindowsAndMessaging::{
            CreateWindowExA, DefWindowProcA, DispatchMessageA, GetMessageA, PostQuitMessage,
            RegisterClassA, TranslateMessage, CW_USEDEFAULT, MSG, WM_APP, WM_DESTROY, WM_LBUTTONUP,
            WM_RBUTTONUP, WNDCLASSA, WS_OVERLAPPEDWINDOW,
        },
    },
};
use zbus::{names::BusName, Connection};

use super::indicator::Indicator;

use crate::proxies::{
    status_notifier_item::StatusNotifierItemProxy,
    status_notifier_watcher::{
        StatusNotifierItemRegisteredStream, StatusNotifierItemUnregisteredStream,
        StatusNotifierWatcherProxy,
    },
};

const WINDOW_CLASS_NAME: &[u8] = b"__hidden__\0";

pub const WMAPP_NOTIFYCALLBACK: u32 = WM_APP + 1;

// thread local storage of StatusNotifierHost such that wndproc can access it.
thread_local! {
    static TX: RefCell<Option<tokio::sync::mpsc::Sender<(WPARAM, LPARAM)>>> = RefCell::new(None)
}

struct HostInner {
    next_id: u16,
    items: HashMap<String, Indicator>,
    by_id: HashMap<u16, String>,
}

#[derive(Clone)]
pub struct StatusNotifierHost {
    connection: Connection,
    hwnd: HWND,
    inner: Arc<Mutex<HostInner>>,
}

impl StatusNotifierHost {
    pub async fn init(connection: &Connection) -> anyhow::Result<()> {
        let mut host = Self {
            connection: connection.clone(),
            hwnd: HWND::default(),
            inner: Arc::new(Mutex::new(HostInner {
                next_id: 0,
                items: HashMap::new(),
                by_id: HashMap::new(),
            })),
        };

        let (tx, mut rx) = tokio::sync::mpsc::channel(1);

        host.hwnd = Self::create_window(tx)?;

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

        {
            let host = host.clone();
            tokio::spawn(async move {
                while let Some((wparam, lparam)) = rx.recv().await {
                    if let Err(e) = host.handle_callback(wparam, lparam).await {
                        log::error!("handle_callback errored: {}", e);
                    }
                }
            });
        }

        // TODO: replace literal with constant
        watcher_proxy
            .register_status_notifier_host("org.freedesktop.impl.portal.desktop.windows")
            .await?;

        Ok(())
    }

    fn create_window(
        tx: tokio::sync::mpsc::Sender<(WPARAM, LPARAM)>,
    ) -> windows::core::Result<HWND> {
        let (send, recv) = mpsc::channel();

        std::thread::spawn(move || {
            TX.with(|c| {
                let _ = c.borrow_mut().insert(tx);
            });

            let instance = unsafe { GetModuleHandleA(None) };
            if instance.0 == 0 {
                send.send(Err(windows::core::Error::from_win32())).unwrap();
                return;
            }

            let window_class = WNDCLASSA {
                hInstance: instance,
                lpfnWndProc: Some(Self::wndproc),
                lpszClassName: PSTR(WINDOW_CLASS_NAME.as_ptr() as _),
                ..Default::default()
            };

            if unsafe { RegisterClassA(&window_class) } == 0 {
                send.send(Err(windows::core::Error::from_win32())).unwrap();
                return;
            }

            let hwnd = unsafe {
                CreateWindowExA(
                    0,
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
            if hwnd.0 == 0 {
                send.send(Err(windows::core::Error::from_win32())).unwrap();
                return;
            }

            send.send(Ok(hwnd)).unwrap();

            // enter message loop
            let mut msg = MSG::default();
            let mut ret: BOOL;
            unsafe {
                loop {
                    ret = GetMessageA(&mut msg, None, 0, 0);
                    if ret == BOOL(0) {
                        break;
                    } else if ret == BOOL(-1) {
                        panic!("{}", windows::core::Error::from_win32());
                    }
                    TranslateMessage(&msg);
                    DispatchMessageA(&msg);
                }
            };
        });

        recv.recv().unwrap()
    }

    unsafe extern "system" fn wndproc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_DESTROY => {
                PostQuitMessage(0);
            }
            WM_CONTEXTMENU => {}
            WMAPP_NOTIFYCALLBACK => TX
                .with(|c| c.borrow().as_ref().unwrap().blocking_send((wparam, lparam)))
                .unwrap(),
            // TODO: might be able to do scroll through WM_INPUT:
            // https://github.com/File-New-Project/EarTrumpet/blob/36e716c7fe4b375274f20229431f0501fe130460/EarTrumpet/UI/Helpers/ShellNotifyIcon.cs#L146
            _ => return DefWindowProcA(hwnd, msg, wparam, lparam),
        };
        LRESULT(0)
    }

    async fn handle_callback(&self, wparam: WPARAM, lparam: LPARAM) -> anyhow::Result<()> {
        let id = (lparam.0 >> 16) as u16;

        let x = (wparam.0 >> 16) as i32;
        let y = (wparam.0 & 0xffff) as i32;

        let indicator = if let Some(indicator) = self.get_item_by_id(id) {
            indicator
        } else {
            bail!("could not find indicator with id: {}", id);
        };

        match (lparam.0 & 0xffff) as u32 {
            WM_LBUTTONUP => indicator.activate(x, y).await.map_err(Into::into),
            WM_RBUTTONUP => indicator.secondary_activate(x, y).await.map_err(Into::into),
            _ => Ok(()),
        }
    }

    fn insert_item(&self, proxy: StatusNotifierItemProxy<'static>) -> anyhow::Result<bool> {
        let mut inner = self.inner.lock().unwrap();
        let id = inner.next_id;
        inner.next_id += 1;
        inner.by_id.insert(id, proxy.destination().to_string());
        Ok(inner
            .items
            .insert(
                proxy.destination().to_string(),
                Indicator::new(self.hwnd, id, proxy)?,
            )
            .is_none())
    }

    fn remove_item(&self, service: &str) -> bool {
        let mut inner = self.inner.lock().unwrap();
        let indicator = inner.items.remove(service);
        indicator
            .map(|i| {
                inner.by_id.remove(&i.id());
                i.unregister();
            })
            .is_some()
    }

    fn get_item_by_id(&self, id: u16) -> Option<Indicator> {
        let inner = self.inner.lock().unwrap();
        let service = inner.by_id.get(&id)?;
        inner.items.get(service).cloned()
    }

    async fn handle_item_registered(
        self,
        mut stream: StatusNotifierItemRegisteredStream<'_>,
    ) -> anyhow::Result<()> {
        while let Some(signal) = stream.next().await {
            let args = signal.args()?;

            log::debug!("handle_item_registered: {}", args.service());

            let proxy = StatusNotifierItemProxy::builder(&self.connection)
                .destination(BusName::try_from(args.service().to_string())?)?
                .build()
                .await?;

            if let Err(e) = self.insert_item(proxy) {
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

            log::debug!("handle_item_unregistered: {}", args.service());

            if self.remove_item(args.service()) {
                log::debug!("{} removed", args.service());
            }
        }

        Ok(())
    }
}
