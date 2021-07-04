use anyhow::Result;
use common::SOCKET_PATH;
use daemon::{run_daemon, start_daemon};
use std::env;
use std::io::{prelude::*, stdout};
use std::os::unix::net::UnixStream;
use std::process::exit;
use std::thread::sleep;
use std::time::Duration;

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

    // otherwise, try to start or connect to the daemon
    loop {
        start_daemon()?;
        // wait for daemon to start
        sleep(Duration::new(0, 10_000_000));

        // listen for a server
        let attempt = UnixStream::connect(SOCKET_PATH);

        // try again
        if attempt.is_err() {
            continue;
        }

        // receive env variables from daemon
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
