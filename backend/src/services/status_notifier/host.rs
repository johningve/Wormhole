// Copyright (c) 2022 John Ingve Olsen
//
// This software is released under the MIT License.
// https://opensource.org/licenses/MIT

use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{mpsc, Arc, Mutex},
};

use anyhow::bail;
use windows::Win32::{
    Foundation::{BOOL, HWND, LPARAM, LRESULT, PSTR, WPARAM},
    System::LibraryLoader::GetModuleHandleA,
    UI::WindowsAndMessaging::{
        CreateWindowExA, DefWindowProcA, DispatchMessageA, GetMessageA, GetSystemMetrics,
        PostQuitMessage, RegisterClassA, SetForegroundWindow, TrackPopupMenuEx, TranslateMessage,
        CW_USEDEFAULT, HMENU, MSG, SM_MENUDROPALIGNMENT, TPM_LEFTALIGN, TPM_RIGHTALIGN,
        TPM_RIGHTBUTTON, WM_APP, WM_COMMAND, WM_DESTROY, WM_LBUTTONUP, WM_RBUTTONUP, WNDCLASSA,
        WS_OVERLAPPEDWINDOW,
    },
};
use zbus::names::{BusName, OwnedBusName};
use zvariant::{ObjectPath, OwnedObjectPath};

use super::{indicator::Indicator, menu::Win32Menu};

use crate::{hiword, loword, proxies::status_notifier_item::StatusNotifierItemProxy};

const WINDOW_CLASS_NAME: &[u8] = b"__hidden__\0";

pub const WMAPP_NOTIFYCALLBACK: u32 = WM_APP + 1;
pub const WMAPP_SHOWMENU: u32 = WM_APP + 2;

// this effectively allocates 100 menu ids per application.
// should be enough :^)
pub const MENU_IDS_PER_APP: u16 = 100;

thread_local! {
    static TX: RefCell<Option<tokio::sync::mpsc::Sender<(u32, WPARAM, LPARAM)>>> = RefCell::new(None)
}

#[derive(Clone, Eq, PartialEq, Hash)]
struct IndicatorID {
    destination: OwnedBusName,
    path: OwnedObjectPath,
}

impl IndicatorID {
    fn new(destination: &BusName<'_>, path: &ObjectPath<'_>) -> Self {
        Self {
            destination: destination.to_owned().into(),
            path: path.to_owned().into(),
        }
    }
}

impl ToString for IndicatorID {
    fn to_string(&self) -> String {
        format!("{}{}", self.destination, self.path.as_str())
    }
}

struct HostInner {
    next_id: u16,
    items: HashMap<IndicatorID, Indicator>,
    by_id: HashMap<u16, IndicatorID>,
}

#[derive(Clone)]
pub struct StatusNotifierHost {
    hwnd: HWND,
    inner: Arc<Mutex<HostInner>>,
}

impl StatusNotifierHost {
    pub async fn new() -> anyhow::Result<Self> {
        let mut host = StatusNotifierHost {
            hwnd: HWND::default(),
            inner: Arc::new(Mutex::new(HostInner {
                next_id: 0,
                items: HashMap::new(),
                by_id: HashMap::new(),
            })),
        };

        let (tx, mut rx) = tokio::sync::mpsc::channel(1);

        host.hwnd = Self::create_window(tx)?;

        {
            let host = host.clone();
            tokio::spawn(async move {
                while let Some((msg, wparam, lparam)) = rx.recv().await {
                    if let Err(e) = match msg {
                        WMAPP_NOTIFYCALLBACK => host.handle_notify_callback(wparam, lparam).await,
                        WM_COMMAND => host.handle_command(wparam, lparam).await,
                        _ => Ok(()),
                    } {
                        log::error!("handle_callback errored: {}", e);
                    }
                }
            });
        }

        Ok(host)
    }

    fn create_window(
        notify_tx: tokio::sync::mpsc::Sender<(u32, WPARAM, LPARAM)>,
    ) -> windows::core::Result<HWND> {
        // channel to communicate with window thread
        let (tx, rx) = mpsc::channel();

        // must create a new thread for window
        std::thread::spawn(move || {
            // install the sender for the notify callback channel
            TX.with(|c| {
                let _ = c.borrow_mut().insert(notify_tx);
            });

            let instance = unsafe { GetModuleHandleA(None) };
            if instance.0 == 0 {
                tx.send(Err(windows::core::Error::from_win32())).unwrap();
                return;
            }

            let window_class = WNDCLASSA {
                hInstance: instance,
                lpfnWndProc: Some(Self::wndproc),
                lpszClassName: PSTR(WINDOW_CLASS_NAME.as_ptr() as _),
                ..Default::default()
            };

            if unsafe { RegisterClassA(&window_class) } == 0 {
                tx.send(Err(windows::core::Error::from_win32())).unwrap();
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
                tx.send(Err(windows::core::Error::from_win32())).unwrap();
                return;
            }

            tx.send(Ok(hwnd)).unwrap();

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

        rx.recv().unwrap()
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
            // pass the following to tokio task for async stuff
            WM_COMMAND | WMAPP_NOTIFYCALLBACK => Self::handle_async(msg, wparam, lparam),
            WMAPP_SHOWMENU => {
                // TODO: not sure what effect this has
                SetForegroundWindow(hwnd);

                let flags = TPM_RIGHTBUTTON
                    | if GetSystemMetrics(SM_MENUDROPALIGNMENT) != 0 {
                        TPM_RIGHTALIGN
                    } else {
                        TPM_LEFTALIGN
                    };

                let menu = Win32Menu::from_handle(HMENU(wparam.0 as _));

                if let Err(e) = TrackPopupMenuEx(
                    menu.handle(),
                    flags,
                    hiword!(lparam.0) as _,
                    loword!(lparam.0) as _,
                    hwnd,
                    std::ptr::null(),
                )
                .ok()
                {
                    log::error!("TrackPopupMenuEx error: {}", e);
                }
            }
            // TODO: might be able to do scroll through WM_INPUT:
            // https://github.com/File-New-Project/EarTrumpet/blob/36e716c7fe4b375274f20229431f0501fe130460/EarTrumpet/UI/Helpers/ShellNotifyIcon.cs#L146
            _ => return DefWindowProcA(hwnd, msg, wparam, lparam),
        };
        LRESULT(0)
    }

    /// handle_async is a helper for wndproc that sends the message to an async task where it can be handled instead.
    fn handle_async(msg: u32, wparam: WPARAM, lparam: LPARAM) {
        TX.with(|c| {
            if let Some(tx) = c.borrow().as_ref() {
                tx.blocking_send((msg, wparam, lparam)).unwrap()
            }
        })
    }

    async fn handle_notify_callback(&self, wparam: WPARAM, lparam: LPARAM) -> anyhow::Result<()> {
        let id = hiword!(lparam.0) as u16;

        let x = loword!(wparam.0) as i32;
        let y = hiword!(wparam.0) as i32;

        let indicator = if let Some(indicator) = self.get_item_by_id(id) {
            indicator
        } else {
            bail!("could not find indicator with id: {}", id);
        };

        match loword!(lparam.0) as u32 {
            WM_LBUTTONUP => indicator.activate(x, y).await.map_err(Into::into),
            WM_RBUTTONUP => indicator.secondary_activate(x, y).await.map_err(Into::into),
            _ => Ok(()),
        }
    }

    async fn handle_command(&self, wparam: WPARAM, lparam: LPARAM) -> anyhow::Result<()> {
        // check what kind of command we are handling
        match hiword!(wparam.0) {
            0 => {}                                             // menu
            1 => bail!("accelerators not implemented"),         // accelerator
            _ => bail!("unexpected control notification code"), // control
        };

        // now we are sure that it is a menu command.
        let menu_id = loword!(wparam.0) as u16;
        let id = menu_id / MENU_IDS_PER_APP;

        let indicator = if let Some(indicator) = self.get_item_by_id(id) {
            indicator
        } else {
            bail!("could not find indicator with id: {}", id);
        };

        indicator.dispatch_menu_command(menu_id).await?;

        Ok(())
    }

    pub fn insert_item(&self, proxy: StatusNotifierItemProxy<'static>) -> anyhow::Result<bool> {
        let mut inner = self.inner.lock().unwrap();

        let dest = IndicatorID::new(proxy.destination(), proxy.path());

        // won't overwrite
        if inner.items.contains_key(&dest) {
            return Ok(false);
        }

        let id = inner.next_id;
        inner.next_id += 1;

        inner.by_id.insert(id, dest.clone());
        inner
            .items
            .insert(dest, Indicator::new(self.hwnd, id, proxy)?);

        Ok(true)
    }

    pub fn handle_service_disappeared(&self, service: &str) -> Vec<String> {
        let mut inner = self.inner.lock().unwrap();

        let mut removed = vec![];

        for k in inner.items.keys() {
            if k.destination.as_str() == service {
                removed.push(k.clone());
            }
        }

        for k in &removed {
            if let Some(i) = inner.items.remove(k) {
                inner.by_id.remove(&i.id());
                i.unregister();
            }
        }

        removed.iter().map(ToString::to_string).collect()
    }

    pub fn registered_items(&self) -> Vec<String> {
        let inner = self.inner.lock().unwrap();
        inner.items.keys().map(ToString::to_string).collect()
    }

    fn get_item_by_id(&self, id: u16) -> Option<Indicator> {
        let inner = self.inner.lock().unwrap();
        let service = inner.by_id.get(&id)?;
        inner.items.get(service).cloned()
    }
}
