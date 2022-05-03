use once_cell::sync::OnceCell;
use serde::Deserialize;
use single_instance::SingleInstance;
use tokio::{io::AsyncReadExt, net::TcpStream};
use util::vmcompute;
use windows::Win32::{
    System::Com::{CoInitializeEx, COINIT_MULTITHREADED},
    UI::HiDpi::{SetProcessDpiAwareness, PROCESS_PER_MONITOR_DPI_AWARE},
};
use zvariant::Type;

use crate::util::vmsocket::HyperVSocket;

mod proxies;
mod services;
#[macro_use]
mod util;

static CONFIG_INSTANCE: OnceCell<Config> = OnceCell::new();

const WELL_KNOWN_NAMES: &[&str] = &[
    "org.freedesktop.impl.portal.desktop.windows",
    "org.kde.StatusNotifierWatcher",
];

pub const REGISTRY_ROOT_KEY: &str = "Software\\DesktopPortalWSL";

#[derive(Debug)]
pub struct Config {
    distro_name: String,
}

impl Config {
    pub fn global() -> &'static Self {
        CONFIG_INSTANCE.get().expect("config is not initialized")
    }

    pub fn distro_name(&self) -> &str {
        &self.distro_name
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let instance = SingleInstance::new(&format!("xdp-wsl-{}", std::env::var("USERNAME")?))?;
    if !instance.is_single() {
        log::error!("Another instance is already running!");
        return Ok(());
    }

    // The process DPI awareness must be set, otherwise shell file dialogs will not be DPI aware either.
    unsafe { SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE) }.unwrap();
    // Initialize the COM library.
    unsafe { CoInitializeEx(std::ptr::null_mut(), COINIT_MULTITHREADED) }.unwrap();

    // Connect to the bridge.
    let mut stream = HyperVSocket::connect(vmcompute::get_wsl_vmid()?, 7070)?;

    // Read the header from the stream.
    let distro_info = read_header(&mut stream).await?;

    CONFIG_INSTANCE
        .set(Config {
            distro_name: distro_info.distro_name.to_string(),
        })
        .unwrap();

    let connection = zbus::ConnectionBuilder::socket(stream)
        .wsl_uid(distro_info.uid)
        .internal_executor(false)
        .build()
        .await?;

    let handle = {
        let connection = connection.clone();
        tokio::spawn(async move {
            loop {
                connection.executor().tick().await;
            }
        })
    };

    for name in WELL_KNOWN_NAMES {
        connection.request_name(*name).await?;
    }

    services::init_all(&connection).await?;

    log::info!("all services initialized");

    handle.await?;

    for name in WELL_KNOWN_NAMES {
        connection.release_name(*name).await?;
    }

    Ok(())
}

async fn read_header(stream: &mut TcpStream) -> anyhow::Result<DistroInfo> {
    // read the header size
    let mut size_buffer: [u8; 4] = [0; 4];
    stream.read_exact(&mut size_buffer).await?;

    // read the header
    let mut header_buffer = vec![0; u32::from_le_bytes(size_buffer) as _];
    stream.read_exact(&mut header_buffer).await?;

    let ctxt = zvariant::EncodingContext::<byteorder::NativeEndian>::new_dbus(0);
    let distro_info: DistroInfo = zvariant::from_slice(&header_buffer, ctxt)?;

    Ok(distro_info)
}

#[derive(Deserialize, Type)]
struct DistroInfo {
    pub distro_name: String,
    pub uid: u32,
}
