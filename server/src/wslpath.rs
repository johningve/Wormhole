use anyhow::bail;
use bindings::Windows::Win32::Storage::FileSystem::GetLogicalDrives;
use regex::Regex;
use std::path::{Component, Path, PathBuf, Prefix};

const WSL_DOMAIN: &str = "\\\\wsl.localhost";

pub fn to_windows(distro: &str, mut wsl_path: &str) -> PathBuf {
    let mut win_path = PathBuf::new();

    if wsl_path.is_empty() {
        return win_path;
    }

    let mnt_reg = Regex::new(r"^/mnt/([A-Za-z])(?:/|$)").unwrap();

    if let Some(captures) = mnt_reg.captures(wsl_path) {
        let letter = captures[1].chars().next().unwrap();
        if LogicalDrives::get().is_present(letter as _) {
            win_path.push(PathBuf::from(format!("{}:\\", letter.to_ascii_uppercase())));
            wsl_path = &wsl_path[captures.get(0).unwrap().end()..];
        }
    } else {
        win_path.push(WSL_DOMAIN);
        win_path.push(distro);
    }

    for part in wsl_path.split('/') {
        win_path.push(part);
    }

    win_path
}

pub fn to_wsl(distro: &str, win_path: &Path) -> anyhow::Result<String> {
    let mut wsl_path = String::new();
    for component in win_path.components() {
        match component {
            Component::Prefix(pfx) => match pfx.kind() {
                Prefix::Disk(letter) => {
                    wsl_path.push_str(&format!("/mnt/{}", letter.to_ascii_lowercase() as char))
                }
                Prefix::UNC(server, share) => {
                    if server.to_string_lossy() != "wsl.localhost"
                        || share.to_string_lossy() != distro
                    {
                        bail!("network share not supported: {:?}", component);
                    }
                }
                _ => bail!("unsupported path prefix: {:?}", component),
            },
            Component::RootDir => wsl_path.push('/'),
            Component::Normal(c) => {
                if !wsl_path.ends_with('/') {
                    wsl_path.push('/');
                }
                wsl_path.push_str(&c.to_string_lossy());
            }
            _ => bail!("unsupported path component: {:?}", component),
        }
    }
    Ok(wsl_path)
}

pub fn get_temp_copy(distro: &str, wsl_file_path: &str) -> std::io::Result<PathBuf> {
    let win_src_path = to_windows(distro, wsl_file_path);

    let mut network_share = PathBuf::from(WSL_DOMAIN);
    network_share.push(distro);

    if !win_src_path.starts_with(&network_share) {
        return Ok(win_src_path);
    }

    let mut win_dest_path = std::env::temp_dir();
    win_dest_path.push("WSLPortal");
    win_dest_path.push(win_src_path.strip_prefix(&network_share).unwrap());

    let src_metadata = std::fs::metadata(&win_src_path)?;
    let dest_metadata = std::fs::metadata(&win_dest_path).ok();

    if dest_metadata.is_none() || src_metadata.modified()? > dest_metadata.unwrap().modified()? {
        std::fs::create_dir_all(win_dest_path.parent().unwrap())?;
        std::fs::copy(&win_src_path, &win_dest_path)?;
    }

    Ok(win_dest_path)
}

struct LogicalDrives(u32);

impl LogicalDrives {
    fn get() -> Self {
        Self(unsafe { GetLogicalDrives() })
    }

    fn is_present(&self, letter: u8) -> bool {
        if !letter.is_ascii_alphabetic() {
            return false;
        }
        let index = letter.to_ascii_uppercase() - b'A';
        if self.0 & (1 << index) != 0 {
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logical_drives_is_present() {
        assert!(LogicalDrives(4).is_present(b'c'));
        assert!(LogicalDrives(1).is_present(b'A'));
    }

    #[test]
    fn test_wsl_path_to_windows() {
        assert_eq!(
            to_windows("Ubuntu", "/mnt/asdf/foo.txt"),
            PathBuf::from("\\\\wsl.localhost\\Ubuntu\\mnt\\asdf\\foo.txt")
        );
        assert_eq!(
            to_windows("Ubuntu", "/mnt/c/Users/"),
            PathBuf::from("C:\\Users")
        );
    }

    #[test]
    fn test_win_path_to_wsl() {
        assert_eq!(
            to_wsl("Ubuntu", &PathBuf::from("C:\\Users\\admin")).unwrap(),
            "/mnt/c/Users/admin"
        );
        assert_eq!(
            to_wsl(
                "Ubuntu",
                &PathBuf::from("\\\\wsl.localhost\\Ubuntu\\home\\admin")
            )
            .unwrap(),
            "/home/admin"
        );
    }
}
