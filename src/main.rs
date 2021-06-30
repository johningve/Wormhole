use anyhow::Result;
use common::SOCKET_PATH;
use daemon::start_daemon;
use std::io::{prelude::*, stdout};
use std::os::unix::net::UnixStream;
use std::thread::sleep;
use std::time::Duration;

mod common;
mod daemon;

fn main() -> Result<()> {
    loop {
        start_daemon();
        sleep(Duration::new(0, 10_000_000));

        // listen for a server
        let attempt = UnixStream::connect(SOCKET_PATH);

        if attempt.is_err() {
            continue;
        }

        match run_client(&mut attempt.unwrap()) {
            Ok(_) => break,
            Err(e) => {
                eprintln!("{}", e);
            }
        }
    }

    Ok(())
}

fn run_client(socket: &mut UnixStream) -> Result<()> {
    socket
        .set_read_timeout(Some(Duration::new(0, 100_000_000)))
        .expect("Failed to set timeout");

    let mut buffer = String::new();
    socket.read_to_string(&mut buffer)?;
    stdout().write_all(buffer.as_bytes())?;
    Ok(())
}
