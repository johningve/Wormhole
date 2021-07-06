use anyhow::{bail, Result};
use common::SOCKET_PATH;
use daemon::{run_daemon, start_daemon};
use std::io::{prelude::*, ErrorKind};
use std::os::unix::net::UnixStream;
use std::process::exit;
use std::thread::sleep;
use std::time::{Duration, SystemTime};
use std::{env, io};

mod common;
mod daemon;

fn main() -> Result<()> {
    // check if "daemon" has been passed as an argument
    if let Some(arg) = env::args().nth(1) {
        if arg != "daemon" {
            eprintln!("usage: {} [daemon]", env::args().nth(0).unwrap());
            exit(1);
        }
        run_daemon()?
    }

    let mut child = start_daemon()?;

    let start_time = SystemTime::now();
    loop {
        // make sure that we don't hang for longer than a second before giving up
        if start_time.elapsed()? > Duration::new(1, 0) {
            bail!("could not connect to daemon");
        }

        // ensure that the daemon process hasn't crashed
        if let Ok(Some(exit_status)) = child.try_wait() {
            let code = exit_status.code().unwrap();
            if code != 0 {
                if let Ok(output) = child.wait_with_output() {
                    io::stdout().write_all(&output.stdout).unwrap();
                    io::stderr().write_all(&output.stderr).unwrap();
                }
                bail!("daemon exited with non-zero exit code: {}", code);
            }
        }

        // wait for daemon to start
        sleep(Duration::new(0, 10_000_000));

        match UnixStream::connect(SOCKET_PATH) {
            Ok(mut stream) => {
                run_client(&mut stream)?;
                break;
            }
            Err(e) => {
                if e.kind() != ErrorKind::ConnectionRefused {
                    bail!("an unknown error occurred: {}", e);
                }
            }
        };
    }

    Ok(())
}

fn run_client(socket: &mut UnixStream) -> Result<()> {
    socket
        .set_read_timeout(Some(Duration::new(0, 100_000_000)))
        .expect("Failed to set timeout");

    let mut buffer = String::new();
    socket.read_to_string(&mut buffer)?;
    io::stdout().write_all(buffer.as_bytes())?;
    Ok(())
}
