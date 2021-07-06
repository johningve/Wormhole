use anyhow::{anyhow, Result};
use common::SOCKET_PATH;
use daemon::{run_daemon, start_daemon};
use std::io::{prelude::*, stdout, ErrorKind};
use std::os::unix::fs::FileTypeExt;
use std::os::unix::net::UnixStream;
use std::process::exit;
use std::thread::sleep;
use std::time::{Duration, SystemTime};
use std::{env, fs};

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

    // TODO: maybe check the single-instance here too?
    let mut child = start_daemon()?;

    let start_time = SystemTime::now();

    loop {
        // make sure that we don't hang for longer than a second before giving up
        if start_time.elapsed()? > Duration::new(1, 0) {
            return Err(anyhow!("could not connect to daemon"));
        }

        // ensure that the daemon process hasn't crashed
        if let Ok(Some(exit_status)) = child.try_wait() {
            let code = exit_status.code().unwrap();
            if code != 0 {
                return Err(anyhow!("daemon exited with non-zero exit code: {}", code));
            }
        }

        // wait for daemon to start
        sleep(Duration::new(0, 10_000_000));

        // wait until a socket is created
        match fs::metadata(SOCKET_PATH) {
            Ok(metadata) => {
                let file_type = metadata.file_type();
                if !file_type.is_socket() {
                    continue;
                }
                match UnixStream::connect(SOCKET_PATH) {
                    Ok(mut stream) => {
                        run_client(&mut stream)?;
                        break;
                    }
                    Err(e) => {
                        if e.kind() != ErrorKind::ConnectionRefused {
                            return Err(anyhow!("an unknown error occurred: {}", e));
                        }
                    }
                };
            }
            Err(e) => {
                if e.kind() != ErrorKind::NotFound {
                    return Err(anyhow!("an unknown error occurred: {}", e));
                }
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
