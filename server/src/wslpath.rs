use bindings::Windows::Win32::Storage::FileSystem::GetLogicalDrives;
use regex::Regex;
use std::path::PathBuf;

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
        win_path.push(PathBuf::from(format!("\\\\wsl.localhost\\{}", distro)));
    }

    for part in wsl_path.split('/') {
        win_path.push(part);
    }

    win_path
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
}
