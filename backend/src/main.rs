use once_cell::sync::OnceCell;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
use util::{vmcompute, vmsocket, wslpath};
use windows::Win32::{
    System::Com::{CoInitializeEx, COINIT_MULTITHREADED},
    UI::HiDpi::{SetProcessDpiAwareness, PROCESS_PER_MONITOR_DPI_AWARE},
};

mod proxies;
mod services;
#[macro_use]
mod util;

static CONFIG_INSTANCE: OnceCell<Config> = OnceCell::new();
static WELL_KNOWN_NAMES: &[&str] = &[
    "org.freedesktop.impl.portal.desktop.windows",
    "org.kde.StatusNotifierWatcher",
];

#[derive(Debug)]
pub struct Config {
    distro_name: String,
    user_name: String,
}

impl Config {
    pub fn global() -> &'static Self {
        CONFIG_INSTANCE.get().expect("config is not initialized")
    }

    pub fn distro_name(&self) -> &str {
        &self.distro_name
    }

    pub fn user_name(&self) -> &str {
        &self.user_name
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    unsafe { SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE) }.unwrap();
    unsafe { CoInitializeEx(std::ptr::null_mut(), COINIT_MULTITHREADED) }.unwrap();

    let mut stream = vmsocket::HyperVSocket::connect(vmcompute::get_wsl_vmid()?, 7070)?;

    // read the header size
    let mut size_buffer: [u8; 4] = [0; 4];
    stream.read_exact(&mut size_buffer).await?;

    // read the header
    let mut header_buffer = vec![0; u32::from_le_bytes(size_buffer) as _];
    stream.read_exact(&mut header_buffer).await?;

    // read the individual header fields
    let mut distro_name = String::new();
    let mut user_name = String::new();
    let mut home_path = String::new();

    let mut offset;
    offset = (&header_buffer[..]).read_line(&mut distro_name).await?;
    offset += (&header_buffer[offset..]).read_line(&mut user_name).await?;
    (&header_buffer[offset..]).read_line(&mut home_path).await?;

    distro_name = distro_name.trim_end().to_string();
    user_name = user_name.trim_end().to_string();
    home_path = home_path.trim_end().to_string();

    CONFIG_INSTANCE
        .set(Config {
            distro_name: distro_name.clone(),
            user_name: user_name.clone(),
        })
        .unwrap();

    // TODO: might want to make this a constant in a shared package
    stream.write_all(b"connect\r\n").await?;

    std::env::set_var("ZBUS_WSL_HOME", wslpath::to_windows(&home_path));
    std::env::set_var("ZBUS_WSL_USER", user_name);

    let connection = zbus::ConnectionBuilder::socket(stream)
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
