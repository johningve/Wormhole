use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{mpsc, Arc, Mutex},
};

use futures::StreamExt;
use windows::Win32::{
    Foundation::{BOOL, HWND, LPARAM, LRESULT, PSTR, WPARAM},
    System::LibraryLoader::GetModuleHandleA,
    UI::{
        Controls::RichEdit::WM_CONTEXTMENU,
        Shell::{NINF_KEY, NIN_SELECT},
        WindowsAndMessaging::{
            CreateWindowExA, DefWindowProcA, DispatchMessageA, GetMessageA, PostQuitMessage,
            RegisterClassA, TranslateMessage, CW_USEDEFAULT, MSG, WM_DESTROY, WM_LBUTTONUP,
            WM_MBUTTONUP, WM_RBUTTONUP, WM_VSCROLL, WNDCLASSA, WS_OVERLAPPEDWINDOW,
        },
    },
};
use zbus::Connection;

use super::systray::SysTrayIcon;

use crate::proxies::status_notifier_watcher::{
    StatusNotifierItemRegisteredStream, StatusNotifierItemUnregisteredStream,
    StatusNotifierWatcherProxy,
};

const WINDOW_CLASS_NAME: &[u8] = b"__hidden__\0";

// thread local storage of StatusNotifierHost such that wndproc can access it.
thread_local! {
    static HOST: RefCell<Option<StatusNotifierHost>> = RefCell::new(None)
}

struct HostInner {
    items: HashMap<String, SysTrayIcon>,
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

    fn create_window(&self) -> windows::core::Result<HWND> {
        let (mut send, recv) = mpsc::channel();

        let host = self.clone();

        std::thread::spawn(move || {
            HOST.with(|c| {
                let _ = c.borrow_mut().insert(host);
            });

            let instance = unsafe { GetModuleHandleA(None) };
            if instance.0 == 0 {
                send.send(Err(windows::core::Error::from_win32())).unwrap();
                return;
            }

            let mut window_class = WNDCLASSA::default();
            window_class.hInstance = instance;
            window_class.lpfnWndProc = Some(Self::wndproc);
            window_class.lpszClassName = PSTR(WINDOW_CLASS_NAME.as_ptr() as _);
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
                return LRESULT(0);
            }
            WM_LBUTTONUP => {}
            WM_MBUTTONUP => {}
            WM_RBUTTONUP => {}
            WM_CONTEXTMENU => {}
            NIN_SELECT | NINF_KEY => {}
            // TODO: might be able to do scroll through WM_INPUT:
            // https://github.com/File-New-Project/EarTrumpet/blob/36e716c7fe4b375274f20229431f0501fe130460/EarTrumpet/UI/Helpers/ShellNotifyIcon.cs#L146
            _ => DefWindowProcA(hwnd, msg, wparam, lparam),
        }
    }

    fn insert_item(&self, service: &str) -> anyhow::Result<bool> {
        let mut inner = self.inner.lock().unwrap();
        // Ok(inner
        //     .items
        //     .insert(
        //         service.to_string(),
        //         Indicator::new(&inner.connection, service)?,
        //     )
        //     .is_none())
        Ok(false)
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
