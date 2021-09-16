fn main() {
    windows::build! {
        Windows::Data::Xml::Dom::XmlDocument,
        Windows::UI::Notifications::{
            ToastNotification, ToastNotificationManager, ToastNotificationHistory, ToastNotifier, ToastActivatedEventArgs,
            ToastDismissedEventArgs, ToastFailedEventArgs,
        },
        Windows::Win32::Networking::WinSock::*,
        Windows::Win32::System::LibraryLoader::*,
        Windows::Win32::System::Memory::LocalFree,
    };
}
