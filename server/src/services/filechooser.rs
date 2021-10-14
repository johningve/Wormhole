use std::{ffi::OsString, os::windows::prelude::OsStringExt, path::PathBuf};

use bindings::Windows::Win32::{
    System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_ALL},
    UI::Shell::{IFileDialogCustomize, IFileOpenDialog, SIGDN_FILESYSPATH},
};
use scopeguard::defer;
use widestring::WideCStr;
use windows::Guid;
use windows::Interface;

// TODO: replace when windows-rs provides these instead.
const CLSID_FILE_OPEN_DIALOG: &str = "DC1C5A9C-E88A-4dde-A5A1-60F82A20AEF7";
const CLSID_FILE_SAVE_DIALOG: &str = "C0B4E2F3-BA21-4773-8DBA-335EC946EB8B";

pub fn get_open_file_name() -> windows::Result<PathBuf> {
    unsafe {
        let dialog: IFileOpenDialog =
            CoCreateInstance(&Guid::from(CLSID_FILE_OPEN_DIALOG), None, CLSCTX_ALL)?;

        let dialog_custom: IFileDialogCustomize = dialog.cast()?;
        dialog_custom.AddText(0, "W1ND0W5 W45 H4CK3D")?;
        dialog_custom.AddCheckButton(1, "cool", false)?;

        dialog.Show(None)?;
        let result = dialog.GetResult()?;

        let path_raw = result.GetDisplayName(SIGDN_FILESYSPATH)?;
        let path = PathBuf::from(WideCStr::from_ptr_str(path_raw.0).to_os_string());
        CoTaskMemFree(path_raw.0 as _);

        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use bindings::Windows::Win32::{
        System::Com::{CoInitializeEx, COINIT_MULTITHREADED},
        UI::HiDpi::{SetProcessDpiAwareness, PROCESS_PER_MONITOR_DPI_AWARE},
    };

    use super::get_open_file_name;

    #[test]
    fn test_get_open_file_name() {
        unsafe { SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE) }.unwrap();
        unsafe { CoInitializeEx(std::ptr::null_mut(), COINIT_MULTITHREADED) }.unwrap();
        println!(
            "{}",
            get_open_file_name().unwrap().as_os_str().to_string_lossy()
        );
    }
}
