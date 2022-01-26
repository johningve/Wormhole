use serde::{Deserialize, Serialize};
use zbus::dbus_proxy;
use zvariant::ObjectPath;
use zvariant_derive::{OwnedValue, Type, Value};

#[dbus_proxy(
    interface = "org.freedesktop.StatusNotifierItem",
    default_path = "/StatusNotifierItem"
)]
pub trait StatusNotifierItem {
    /// Describes the category of this item.
    #[dbus_proxy(property)]
    fn category(&self) -> zbus::Result<String>;

    /// It's a name that should be unique for this application and consistent between sessions,
    /// such as the application name itself.
    #[dbus_proxy(property)]
    fn id(&self) -> zbus::Result<String>;

    /// It's a name that describes the application, it can be more descriptive than Id.
    #[dbus_proxy(property)]
    fn title(&self) -> zbus::Result<String>;

    /// Describes the status of this item or of the associated application.
    #[dbus_proxy(property)]
    fn status(&self) -> zbus::Result<String>;

    /// It's the windowing-system dependent identifier for a window.
    /// The application can chose one of its windows to be available through this property
    /// or just set 0 if it's not interested.
    #[dbus_proxy(property)]
    fn window_id(&self) -> zbus::Result<u32>;

    /// The StatusNotifierItem can carry an icon that can be used by the visualization to identify the item.
    #[dbus_proxy(property)]
    fn icon_name(&self) -> zbus::Result<String>;

    /// Carries an ARGB32 binary representation of the icon.
    #[dbus_proxy(property)]
    fn icon_pixmap(&self) -> zbus::Result<Vec<Pixmap>>;

    /// The Freedesktop-compliant name of an icon.
    /// This can be used by the visualization to indicate extra state information,
    /// for instance as an overlay for the main icon.
    #[dbus_proxy(property)]
    fn overlay_icon_name(&self) -> zbus::Result<String>;

    /// ARGB32 binary representation of the overlay icon.
    #[dbus_proxy(property)]
    fn overlay_icon_pixmap(&self) -> zbus::Result<Vec<Pixmap>>;

    /// The Freedesktop-compliant name of an icon.
    /// This can be used by the visualization to indicate that the item is in RequestingAttention state.
    #[dbus_proxy(property)]
    fn attention_icon_name(&self) -> zbus::Result<String>;

    /// ARGB32 binary representation of the requesting attention icon.
    #[dbus_proxy(property)]
    fn attention_icon_pixmap(&self) -> zbus::Result<Vec<Pixmap>>;

    /// An item can also specify an animation associated to the RequestingAttention state.
    /// This should be either a Freedesktop-compliant icon name or a full path.
    /// The visualization can chose between the movie or AttentionIconPixmap
    /// (or using neither of those) at its discretion.
    #[dbus_proxy(property)]
    fn attention_movie_name(&self) -> zbus::Result<String>;

    /// Data structure that describes extra information associated to this item,
    /// that can be visualized for instance by a tooltip (or by any other mean the visualization consider appropriate.
    /// # Components:
    /// 1. Freedesktop-compliant name for an icon.
    /// 2. Icon data.
    /// 3. Title for this tooltip.
    /// 4. Descriptive text for this tooltip. It can contain also a subset of the HTML markup language,
    ///    for a list of allowed tags see Section Markup.
    #[dbus_proxy(property)]
    fn tool_tip(&self) -> zbus::Result<ToolTip>;

    /// The item only support the context menu,
    /// the visualization should prefer showing the menu or sending ContextMenu() instead of Activate().
    #[dbus_proxy(property)]
    fn item_is_menu(&self) -> zbus::Result<bool>;

    /// DBus path to an object which should implement the com.canonical.dbusmenu interface.
    #[dbus_proxy(property)]
    fn menu(&self) -> zbus::Result<ObjectPath<'_>>;

    /// Asks the status notifier item to show a context menu,
    /// this is typically a consequence of user input,
    /// such as mouse right click over the graphical representation of the item.
    ///
    /// The x and y parameters are in screen coordinates
    /// and is to be considered an hint to the item about where to show the context menu.
    fn context_menu(&self, x: i32, y: i32) -> zbus::Result<()>;

    /// Asks the status notifier item for activation,
    /// this is typically a consequence of user input,
    /// such as mouse left click over the graphical representation of the item.
    /// The application will perform any task is considered appropriate as an activation request.
    ///
    /// The x and y parameters are in screen coordinates
    /// and is to be considered an hint to the item where to show eventual windows (if any).
    fn activate(&self, x: i32, y: i32) -> zbus::Result<()>;

    /// Is to be considered a secondary and less important form of activation compared to Activate.
    /// This is typically a consequence of user input,
    /// such as mouse middle click over the graphical representation of the item.
    /// The application will perform any task is considered appropriate as an activation request.
    ///
    /// The x and y parameters are in screen coordinates
    /// and is to be considered an hint to the item where to show eventual windows (if any).
    fn secondary_activate(&self, x: i32, y: i32) -> zbus::Result<()>;

    /// The user asked for a scroll action.
    /// This is caused from input such as mouse wheel over the graphical representation of the item.
    /// The delta parameter represent the amount of scroll,
    /// the orientation parameter represent the horizontal or vertical orientation of the scroll request
    /// and its legal values are horizontal and vertical.
    fn scroll(&self, delta: i32, orientation: &str) -> zbus::Result<()>;

    /// The item has a new title: the graphical representation should read it again immediately.
    #[dbus_proxy(signal)]
    fn new_title(&self) -> zbus::Result<()>;

    /// The item has a new icon: the graphical representation should read it again immediately.
    #[dbus_proxy(signal)]
    fn new_icon(&self) -> zbus::Result<()>;

    /// The item has a new overlay icon: the graphical representation should read it again immediately.
    #[dbus_proxy(signal)]
    fn new_overlay_icon(&self) -> zbus::Result<()>;

    /// The item has a new attention icon: the graphical representation should read it again immediately.
    #[dbus_proxy(signal)]
    fn new_attention_icon(&self) -> zbus::Result<()>;

    /// The item has a new tooltip: the graphical representation should read it again immediately.
    #[dbus_proxy(signal)]
    fn new_tooltip(&self) -> zbus::Result<()>;

    /// The item has a new status, that is passed as an argument of the signal.
    #[dbus_proxy(signal)]
    fn new_status(&self, status: &str) -> zbus::Result<()>;
}

#[derive(Serialize, Deserialize, Type, Value, OwnedValue)]
pub struct Pixmap {
    pub width: i32,
    pub height: i32,
    pub image_data: Vec<u8>,
}

#[derive(Serialize, Deserialize, Type, Value, OwnedValue)]
pub struct ToolTip {
    pub icon_name: String,
    pub icon_data: Vec<Pixmap>,
    pub title: String,
    pub description: String,
}
