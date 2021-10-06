use bindings::Windows::Win32::Storage::FileSystem::GetLogicalDrives;
use std::path::{Component, Path, PathBuf};
use std::os::windows::ffi::OsStrExt;

pub fn to_windows(wsl_path: impl AsRef<Path>) -> PathBuf {
    let wsl_path = wsl_path.as_ref();
    let mut components = wsl_path.components();
    let mut win_path = PathBuf::new();

    if wsl_path.starts_with("/mnt") {
        if let Some(dir) = wsl_path.components().nth(2) {
            let d = dir.as_os_str().encode_wide().nth(0).unwrap();
        }
    }


    if components.next() != Some(Component::RootDir) {

    }

    if (let Some(dir) = it.next()) == "/mnt" {
        path = path.strip_prefix("/mnt/").unwrap();
        let letter = path.components()
        let drives = LogicalDrives(unsafe { GetLogicalDrives() });
        if drives.is_present(letter) {}
    }
    path.to_owned()
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
    fn test_is_present() {
        assert!(LogicalDrives(4).is_present(b'c'));
        assert!(LogicalDrives(1).is_present(b'A'));
    }
}
