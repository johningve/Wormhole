// Copyright (c) 2022 John Ingve Olsen
//
// This software is released under the MIT License.
// https://opensource.org/licenses/MIT

use anyhow::{anyhow, bail, Context, Result};
use nix::sys::wait::waitpid;
use nix::unistd::{fork, setsid, ForkResult};
use single_instance::SingleInstance;
use std::env;
use std::fs;
use std::io::{BufRead, Write};
use std::os::unix::net::UnixListener;
use std::process::{exit, Command, Stdio};

use crate::common::SOCKET_PATH;

fn dbus_launch() -> Result<(String, u32)> {
    let out = Command::new("dbus-daemon")
        .arg("--fork")
        .arg("--syslog-only")
        .arg("--print-pid")
        .arg("--print-address")
        .arg("--session")
        .output()
        .context("failed to run dbus-daemon")?;

    if !out.status.success() {
        bail!(
            "dbus-daemon exited with code: {}\n{}",
            out.status.code().unwrap(),
            String::from_utf8_lossy(&out.stderr)
        )
    }

    let mut lines_iter = out.stdout.lines();

    // the lines iterator returns a Result<_>, so we need to use the '?' operator twice.
    let addr = lines_iter
        .next()
        .ok_or_else(|| anyhow!("dbus-daemon did not print address to stdout"))?
        .context("failed to read output from dbus-daemon")?;

    // is this silly?
    let pid = lines_iter
        .next()
        .ok_or_else(|| anyhow!("dbus-daemon did not print pid to stdout"))?
        .context("failed to read output from dbus-daemon")?
        .parse::<u32>()
        .context("failed to parse pid")?;

    Ok((addr, pid))
}

pub fn start_daemon() -> Result<()> {
    // double fork such that the daemon is disconnected from the child
    match unsafe { fork() }? {
        ForkResult::Parent { child } => {
            waitpid(child, None)?;
        }
        ForkResult::Child => {
            // setsid creates a new process group id, or something...
            setsid()?;
            Command::new(env::current_exe()?)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .arg("daemon")
                .spawn()?;
            exit(0);
        }
    };
    Ok(())
}

pub fn run_daemon() -> Result<()> {
    let machine_id = machine_uid::get().map_err(|e| anyhow!("failed to get machine id: {}", e))?;

    // we use the machine id in the instance name, because otherwise it is not possible to run
    // wsl-session-manager in multiple distros at the same time.
    let si = SingleInstance::new(format!("wsl-session-manager:{}", machine_id).as_str())?;
    if !si.is_single() {
        exit(0);
    }

    // launch the dbus-daemon and get the output
    let (addr, pid) = dbus_launch()?;

    // set up the socket
    let _ = fs::remove_file(SOCKET_PATH);
    let listener = UnixListener::bind(SOCKET_PATH)?;

    // handle connections
    loop {
        let (mut socket, _) = listener.accept()?;
        // don't care about failed writes
        let _ = socket.write_all(
            format!(
                "DBUS_SESSION_BUS_ADDRESS='{}';
export DBUS_SESSION_BUS_ADDRESS;
DBUS_SESSION_BUS_PID='{}';
export DBUS_SESSION_BUS_PID;",
                &addr, &pid
            )
            .as_ref(),
        );
    }
}
