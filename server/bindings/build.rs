fn main() {
    windows::build! {
        Windows::Data::Xml::Dom::XmlDocument,
        Windows::UI::Notifications::{ToastNotification, ToastNotificationManager, ToastNotifier, ToastActivatedEventArgs},
        Windows::Win32::Networking::WinSock::*,
    };
}
