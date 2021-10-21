fn main() {
    windows::build! {
        Windows::Data::Xml::Dom::XmlDocument,
        Windows::Win32::Networking::WinSock::*,
        Windows::UI::Notifications::{
            ToastNotification, ToastNotificationManager, ToastNotificationHistory, ToastNotifier, ToastActivatedEventArgs,
            ToastDismissedEventArgs, ToastFailedEventArgs,
        },
        Windows::Win32::UI::HiDpi::SetProcessDpiAwareness,
        Windows::Win32::UI::Shell::{
            _FILEOPENDIALOGOPTIONS, IFileOpenDialog, IFileSaveDialog, IFileDialogCustomize, IShellItem, IShellItemArray,
            SHCreateItemFromParsingName,
        },
        Windows::Win32::Storage::FileSystem::GetLogicalDrives,
        Windows::Win32::System::Com::{CoInitializeEx, CoCreateInstance, CoTaskMemFree},
        Windows::Win32::System::LibraryLoader::*,
        Windows::Win32::System::Memory::LocalFree,
    };
}
