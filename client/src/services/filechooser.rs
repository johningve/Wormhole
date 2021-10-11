use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use tonic::transport::Channel;
use zbus::{dbus_interface, Connection};
use zvariant::{ObjectPath, OwnedObjectPath};
use zvariant_derive::{DeserializeDict, SerializeDict, Type, TypeDict};

pub struct FileChooser {}

impl FileChooser {
    pub async fn init(grpc_channel: Channel, dbus_connection: &Connection) -> zbus::Result<()> {
        dbus_connection
            .request_name("org.freedesktop.impl.portal.FileChooser")
            .await?;

        dbus_connection
            .object_server_mut()
            .await
            .at(super::PORTAL_PATH, FileChooser {})?;

        Ok(())
    }
}

#[dbus_interface(name = "org.freedesktop.impl.portal.FileChooser")]
impl FileChooser {
    async fn open_file(
        &self,
        handle: OwnedObjectPath,
        app_id: &str,
        parent_window: &str,
        title: &str,
        options: OpenFileOptions,
    ) {
    }
}

#[derive(Serialize, Deserialize, Type, Clone, Debug)]
/// A file filter, to limit the available file choices to a mimetype or a glob
/// pattern.
pub struct FileFilter(String, Vec<(FilterType, String)>);

#[derive(Serialize_repr, Clone, Deserialize_repr, PartialEq, Debug, Type)]
#[repr(u32)]
#[doc(hidden)]
enum FilterType {
    GlobPattern = 0,
    MimeType = 1,
}

impl FileFilter {
    /// Create a new file filter
    ///
    /// # Arguments
    ///
    /// * `label` - user-visible name of the file filter.
    pub fn new(label: &str) -> Self {
        Self(label.to_string(), vec![])
    }

    /// Adds a mime type to the file filter.
    pub fn mimetype(mut self, mimetype: &str) -> Self {
        self.1.push((FilterType::MimeType, mimetype.to_string()));
        self
    }

    /// Adds a glob pattern to the file filter.
    pub fn glob(mut self, pattern: &str) -> Self {
        self.1.push((FilterType::GlobPattern, pattern.to_string()));
        self
    }
}

#[derive(Serialize, Deserialize, Type, Clone, Debug)]
/// Presents the user with a choice to select from or as a checkbox.
pub struct Choice(String, String, Vec<(String, String)>, String);

impl Choice {
    /// Creates a checkbox choice.
    ///
    /// # Arguments
    ///
    /// * `id` - A unique identifier of the choice.
    /// * `label` - user-visible name of the choice.
    /// * `state` - the initial state value.
    pub fn boolean(id: &str, label: &str, state: bool) -> Self {
        Self::new(id, label, &state.to_string())
    }

    /// Creates a new choice.
    ///
    /// # Arguments
    ///
    /// * `id` - A unique identifier of the choice.
    /// * `label` - user-visible name of the choice.
    /// * `initial_selection` - the initially selected value.
    pub fn new(id: &str, label: &str, initial_selection: &str) -> Self {
        Self(
            id.to_string(),
            label.to_string(),
            vec![],
            initial_selection.to_string(),
        )
    }

    /// Adds a (key, value) as a choice.
    pub fn insert(mut self, key: &str, value: &str) -> Self {
        self.2.push((key.to_string(), value.to_string()));
        self
    }

    /// The choice's unique id
    pub fn id(&self) -> &str {
        &self.0
    }

    /// The user visible label of the choice.
    pub fn label(&self) -> &str {
        &self.1
    }

    /// The initially selected value.
    pub fn initial_selection(&self) -> &str {
        &self.3
    }
}

#[derive(DeserializeDict, SerializeDict, TypeDict, Clone, Debug, Default)]
struct OpenFileOptions {
    accept_label: Option<String>,
    modal: Option<bool>,
    multiple: Option<bool>,
    directory: Option<bool>,
    filters: Vec<FileFilter>,
    current_filter: Option<FileFilter>,
    choices: Vec<Choice>,
}
