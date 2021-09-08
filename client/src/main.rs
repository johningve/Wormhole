use std::error::Error;

use vmsocket::VmSocket;

mod vmsocket;

fn main() -> Result<(), Box<dyn Error>> {
    let mut socket = VmSocket::connect(6000)?;

    Ok(())
}
