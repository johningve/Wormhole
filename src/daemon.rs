use anyhow::{bail, Context, Result};
use single_instance::SingleInstance;
use std::env;
use std::fs;
use std::io::Write;
use std::os::unix::net::UnixListener;
use std::process::{exit, Command};

use crate::common::SOCKET_PATH;

fn dbus_launch() -> Result<String> {
    let out = Command::new("dbus-launch")
        .arg("--exit-with-x11")
        .arg("--auto-syntax")
        .arg("--close-stderr")
        .output()
        .context("failed to run dbus-launch")?;

    if !out.status.success() {
        bail!(
            "dbus-launch exited with code: {}\n{}",
            out.status.code().unwrap(),
            String::from_utf8_lossy(&out.stderr)
        )
    }

    Ok(String::from_utf8_lossy(out.stdout.as_slice()).into())
}

pub fn start_daemon() -> Result<()> {
    Command::new(env::current_exe()?).arg("daemon").spawn()?;
    Ok(())
}

pub fn run_daemon() -> Result<()> {
    let si = SingleInstance::new("wsl-session-manager")?;
    if !si.is_single() {
        exit(0);
    }

    // launch the dbus-daemon and get the output
    let dbus_env = dbus_launch()?;

    // set up the socket
    let _ = fs::remove_file(SOCKET_PATH);
    let listener = UnixListener::bind(SOCKET_PATH)?;

    // handle connections
    loop {
        let (mut socket, _) = listener.accept()?;
        // don't care about failed writes
        let _ = socket.write_all(dbus_env.as_ref());
    }
}
