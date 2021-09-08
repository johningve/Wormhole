mod vmcompute;
mod vmsocket;

fn main() -> std::io::Result<()> {
    let listener =
        vmsocket::bind_hyperv_socket(vmcompute::get_wsl_vmid()?.expect("WSL VM not found!"), 7070);

    Ok(())
}
