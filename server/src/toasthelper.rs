extern crate xml;
use windows::{IInspectable, Interface, HSTRING};
use xml::escape::{escape_str_attribute, escape_str_pcdata};

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
    Windows::Data::Xml::Dom::XmlDocument,
    Windows::Foundation::TypedEventHandler,
    Windows::UI::Notifications::{ToastActivatedEventArgs, ToastFailedEventArgs},
    Windows::UI::Notifications::{ToastDismissalReason, ToastNotification},
    Windows::UI::Notifications::{ToastDismissedEventArgs, ToastNotificationManager},
};

//https://social.msdn.microsoft.com/Forums/Windows/en-US/99e0d4bd-07cb-4ebd-8c92-c44ac6e7e5de/toast-notification-dismissed-event-handler-not-called-every-time?forum=windowsgeneraldevelopmentissues
pub use windows::Error;

use crate::services::notifications::Notification;
use crate::wslpath;

pub struct ToastHelper {
    toast: ToastNotification,
}

impl ToastHelper {
    pub fn new(
        tag: &str,
        distro: &str,
        summary: &str,
        body: &str,
        image: Option<&str>,
        actions: Vec<&str>,
    ) -> anyhow::Result<ToastHelper> {
        let image = if image.is_some() {
            let image_path = wslpath::get_temp_copy(distro, image.unwrap())?;
            log::debug!("using image: {}", image_path.as_os_str().to_string_lossy());
            format!(
                r#"<image placement="appLogoOverride" src="file://{}" />"#,
                escape_str_pcdata(&image_path.as_os_str().to_string_lossy()),
            )
        } else {
            String::new()
        };

        let visual = format!(
            r#"<visual>
                <binding template="ToastGeneric">
                    <text id="1">{heading}</text>
                    <text id="2">{content}</text>
                    {image}
                    <!-- <image placement="appLogoOverride" hint-crop="circle" src="file:///c:/path_to_image_above_toast.jpg" alt="alt text" /> -->
                    <!-- <image placement="Hero" src="file:///C:/path_to_image_in_toast.jpg" alt="alt text2" /> -->
                    <!-- <image id="1" src="file:///..." alt="another_image" /> -->
                </binding>
            </visual>"#,
            heading = escape_str_pcdata(summary),
            content = escape_str_pcdata(body),
            image = image,
        );

        let mut actions_xml = String::from("<actions>");
        let mut launch_arg = "";

        // TODO: the freedesktop notifications spec sends actions in a vector, these should really be paired up since
        // each even index is an action name, and every odd index is a display name.
        for action in actions.chunks_exact(2) {
            if action[0] == "default" {
                launch_arg = "default";
            } else {
                actions_xml.push_str(
                    format!(
                        r#"<action content="{content}" arguments="{action}" />"#,
                        content = escape_str_attribute(action[1]),
                        action = escape_str_attribute(action[0])
                    )
                    .as_str(),
                );
            }
        }
        actions_xml.push_str("</actions>");

        let toast_xml = XmlDocument::new()?;
        let xml = format!(
            r#"<toast duration="long" launch="{launch}">
                {visual}
                <audio src="ms-winsoundevent:Notification.Default" />
                <!-- <audio silent="true" /> -->
                <!-- See https://docs.microsoft.com/en-us/windows/uwp/design/shell/tiles-and-notifications/toast-pending-update?tabs=xml for possible actions -->
                {actions}
            </toast>"#,
            launch = launch_arg,
            visual = visual,
            actions = actions_xml
        );
        toast_xml.LoadXml(xml).expect("the xml is malformed");

        let toast = ToastNotification::CreateToastNotification(toast_xml)?;
        toast.SetTag(tag)?;

        Ok(ToastHelper { toast })
    }

    pub fn on_activated(&self, callback: impl Fn(String) + 'static) -> windows::Result<()> {
        self.toast.Activated(TypedEventHandler::new(
            move |_, result: &Option<IInspectable>| {
                if let Some(result) = result {
                    let args = result.cast::<ToastActivatedEventArgs>()?;
                    callback(args.Arguments()?.to_string());
                }
                Ok(())
            },
        ))?;
        Ok(())
    }

    pub fn on_dismissed(
        &self,
        callback: impl Fn(ToastDismissalReason) + 'static,
    ) -> windows::Result<()> {
        self.toast.Dismissed(TypedEventHandler::new(
            move |_, result: &Option<ToastDismissedEventArgs>| {
                if let Some(_result) = result {
                    callback(_result.Reason().unwrap());
                }
                Ok(())
            },
        ))?;
        Ok(())
    }

    pub fn on_failed(&self, callback: impl Fn(windows::Error) + 'static) -> windows::Result<()> {
        self.toast.Failed(TypedEventHandler::new(
            move |_, result: &Option<ToastFailedEventArgs>| {
                if let Some(result) = result {
                    callback(windows::Error::new(
                        result.ErrorCode().unwrap(),
                        "failed to show ToastNotification",
                    ));
                }
                Ok(())
            },
        ))?;
        Ok(())
    }

    pub fn show(&self) -> windows::Result<()> {
        // If you have a valid app id, (ie installed using wix) then use it here.
        let toast_notifier = ToastNotificationManager::CreateToastNotifierWithId(HSTRING::from(
            "{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\\WindowsPowerShell\\v1.0\\powershell.exe",
        ))?;

        // Show the toast.
        // Note this returns success in every case, including when the toast isn't shown.
        toast_notifier.Show(&self.toast)
    }

    pub fn dismiss(&self) -> windows::Result<()> {
        let notification_history = ToastNotificationManager::History()?;
        notification_history.Remove(self.toast.Tag()?)
    }
}
