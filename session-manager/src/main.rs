use anyhow::{bail, Result};
use common::SOCKET_PATH;
use daemon::{run_daemon, start_daemon};
use std::env;
use std::io::{self, prelude::*, ErrorKind};
use std::os::unix::net::UnixStream;
use std::process::exit;
use std::thread::sleep;
use std::time::{Duration, SystemTime};

mod common;
mod daemon;

#[allow(clippy::iter_nth_zero)]
fn main() -> Result<()> {
    // check if "daemon" has been passed as an argument
    if let Some(arg) = env::args().nth(1) {
        if arg != "daemon" {
            eprintln!("usage: {} [daemon]", env::args().nth(0).unwrap());
            exit(1);
        }
        run_daemon()?
    }

    let start_time = SystemTime::now();
    loop {
        // make sure that we don't hang for longer than a second before giving up
        if start_time.elapsed()? > Duration::new(1, 0) {
            bail!("could not connect to daemon");
        }

        match UnixStream::connect(SOCKET_PATH) {
            Ok(mut stream) => {
                run_client(&mut stream)?;
                break;
            }
            Err(e) if e.kind() == ErrorKind::ConnectionRefused => {}
            Err(e) if e.kind() == ErrorKind::NotFound => {}
            Err(e) => {
                bail!("an unknown error occurred: {}", e);
            }
        };

        // attempt to start the daemon
        start_daemon()?;

        // wait for daemon to start
        sleep(Duration::new(0, 10_000_000));
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
