extern crate xml;
use std::path::Path;
use windows::{IInspectable, Interface, HSTRING};
use xml::escape::escape_str_attribute;

// You need to have the windows crate in your Cargo.toml
//
// and call windows::build! in a build.rs file like so
// // see https://microsoft.github.io/windows-docs-rs/doc/bindings/windows/ for possible bindings
// fn main() {
//     windows::build!(
//         windows::data::xml::dom::XmlDocument,
//         windows::ui::notifications::{ToastNotification, ToastNotificationManager},
//     );
// }
// or have pregenerated code that does the same thing
use bindings::{
    Windows::Data::Xml::Dom::XmlDocument, Windows::Foundation::TypedEventHandler,
    Windows::UI::Notifications::ToastActivatedEventArgs,
    Windows::UI::Notifications::ToastNotification,
    Windows::UI::Notifications::ToastNotificationManager,
};

//https://social.msdn.microsoft.com/Forums/Windows/en-US/99e0d4bd-07cb-4ebd-8c92-c44ac6e7e5de/toast-notification-dismissed-event-handler-not-called-every-time?forum=windowsgeneraldevelopmentissues
pub use windows::Error;

fn do_toast() -> windows::Result<()> {
    let toast_xml = XmlDocument::new()?;

    toast_xml.LoadXml(
        format!(r#"<toast duration="long">
                <visual>
                    <binding template="ToastGeneric">
                        <text id="1">title</text>
                        <text id="2">first line</text>
                        <text id="3">third line</text>
                        <image placement="appLogoOverride" hint-crop="circle" src="file:///c:/path_to_image_above_toast.jpg" alt="alt text" />
                        <image placement="Hero" src="file:///C:/path_to_image_in_toast.jpg" alt="alt text2" />
                        <image id="1" src="file:///{}" alt="another_image" />
                    </binding>
                </visual>
                <audio src="ms-winsoundevent:Notification.Default" />
                <!-- <audio silent="true" /> -->
                <!-- See https://docs.microsoft.com/en-us/windows/uwp/design/shell/tiles-and-notifications/toast-pending-update?tabs=xml for possible actions -->
                <actions>
                    <action content="check" arguments="check" />
                    <action content="cancel" arguments="cancel" />
                </actions>
            </toast>"#,
        escape_str_attribute(&Path::new("C:\\path_to_image_in_toast.jpg").display().to_string()),
    )).expect("the xml is malformed");

    // Create the toast and attach event listeners
    let toast_notification = ToastNotification::CreateToastNotification(toast_xml)?;

    // happens if any of the toasts actions are interacted with (as a popup or in the action center)
    toast_notification.Activated(TypedEventHandler::new(
        |sender, result: &Option<IInspectable>| {
            println!("activated");
            if let Some(result) = result {
                let args = result.cast::<ToastActivatedEventArgs>().unwrap();
                dbg!(args.Arguments());
            }
            Ok(())
        },
    ))?;

    // happens if the toast is moved to the action center or dismissed in the action center
    toast_notification.Dismissed(TypedEventHandler::new(|sender, result| {
        println!("dismissed");
        Ok(())
    }))?;

    // happens if toasts are disabled
    toast_notification.Failed(TypedEventHandler::new(|sender, result| {
        println!("failed");
        Ok(())
    }))?;

    // If you have a valid app id, (ie installed using wix) then use it here.
    let toast_notifier = ToastNotificationManager::CreateToastNotifierWithId(HSTRING::from(
        "{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\\WindowsPowerShell\\v1.0\\powershell.exe",
    ))?;

    // Show the toast.
    // Note this returns success in every case, including when the toast isn't shown.
    toast_notifier.Show(&toast_notification)
}
