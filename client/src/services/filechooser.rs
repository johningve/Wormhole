use rpc::filechooser::{file_chooser_client::FileChooserClient, OpenFileRequest, SaveFileRequest};
use tonic::{codegen::InterceptedService, transport::Channel};
use zbus::{dbus_interface, Connection};
use zvariant::OwnedObjectPath;

use super::interceptor::DistroInterceptor;

pub struct FileChooser {
    remote: FileChooserClient<InterceptedService<Channel, DistroInterceptor>>,
}

impl FileChooser {
    pub async fn init(grpc_channel: Channel, dbus_connection: &Connection) -> zbus::Result<()> {
        dbus_connection.object_server_mut().await.at(
            super::PORTAL_PATH,
            FileChooser {
                remote: FileChooserClient::with_interceptor(grpc_channel, DistroInterceptor {}),
            },
        )?;

        log::info!("FileChooser portal enabled.");

        Ok(())
    }
}

#[dbus_interface(name = "org.freedesktop.impl.portal.FileChooser")]
impl FileChooser {
    async fn open_file(
        &mut self,
        handle: OwnedObjectPath,
        app_id: &str,
        parent_window: &str,
        title: &str,
        options: dbus::OpenFileOptions,
    ) -> (u32, dbus::OpenFileResults) {
        log::debug!("open_file called: ");
        log::debug!("\thandle: {}", handle.as_str());
        log::debug!("\tapp_id: {}", app_id);
        log::debug!("\tparent_window: {}", parent_window);
        log::debug!("\ttitle: {}", title);
        log::debug!("\toptions: {:?}", options);

        match self
            .remote
            .open_file(tonic::Request::new(OpenFileRequest {
                parent_window: parent_window.to_string(),
                title: title.to_string(),
                options: Some(options.into()),
            }))
            .await
        {
            Ok(response) => {
                let result = response.into_inner();
                (0, dbus::OpenFileResults::from(result))
            }
            Err(e) => {
                log::error!("open_file errored: {}", e);
                (1, dbus::OpenFileResults::default())
            }
        }
    }

    async fn save_file(
        &mut self,
        handle: OwnedObjectPath,
        app_id: &str,
        parent_window: &str,
        title: &str,
        options: dbus::SaveFileOptions,
    ) -> (u32, dbus::SaveFileResults) {
        log::debug!("save_file called: ");
        log::debug!("\thandle: {}", handle.as_str());
        log::debug!("\tapp_id: {}", app_id);
        log::debug!("\tparent_window: {}", parent_window);
        log::debug!("\ttitle: {}", title);
        log::debug!("\toptions: {:?}", options);

        match self
            .remote
            .save_file(tonic::Request::new(SaveFileRequest {
                parent_window: parent_window.to_string(),
                title: title.to_string(),
                options: Some(options.into()),
            }))
            .await
        {
            Ok(response) => {
                let result = response.into_inner();
                (0, dbus::SaveFileResults::from(result))
            }
            Err(e) => {
                log::error!("open_file errored: {}", e);
                (1, dbus::SaveFileResults::default())
            }
        }
    }

    async fn save_files(
        &mut self,
        handle: OwnedObjectPath,
        app_id: &str,
        parent_window: &str,
        title: &str,
        options: dbus::SaveFilesOptions,
    ) -> (u32, dbus::SaveFilesResults) {
        (0, dbus::SaveFilesResults::default())
    }
}

mod dbus {
    use std::collections::HashMap;

    use rpc::filechooser as rpc;
    use serde::{Deserialize, Serialize};
    use serde_repr::{Deserialize_repr, Serialize_repr};
    use zvariant_derive::{DeserializeDict, SerializeDict, Type, TypeDict};

    #[derive(Serialize, Deserialize, Type, Clone, Debug)]
    /// A file filter, to limit the available file choices to a mimetype or a glob
    /// pattern.
    pub struct FileFilter(String, Vec<(FilterType, String)>);

    impl From<FileFilter> for rpc::FileFilter {
        fn from(val: FileFilter) -> Self {
            let mut entries = Vec::new();
            for (filter_type, filter) in val.1 {
                entries.push(rpc::file_filter::FilterEntry {
                    r#type: Into::<rpc::FilterType>::into(filter_type) as _,
                    filter,
                });
            }

            rpc::FileFilter {
                label: val.0,
                entries,
            }
        }
    }

    impl From<rpc::FileFilter> for FileFilter {
        fn from(filter: rpc::FileFilter) -> Self {
            let mut entries = Vec::new();
            for entry in filter.entries {
                entries.push((FilterType::from(entry.r#type()), entry.filter));
            }
            Self(filter.label, entries)
        }
    }

    #[derive(Serialize_repr, Clone, Deserialize_repr, PartialEq, Debug, Type)]
    #[repr(u32)]
    #[doc(hidden)]
    enum FilterType {
        GlobPattern = 0,
        MimeType = 1,
    }

    impl From<FilterType> for rpc::FilterType {
        fn from(val: FilterType) -> Self {
            match val {
                FilterType::GlobPattern => rpc::FilterType::GlobPattern,
                FilterType::MimeType => rpc::FilterType::MimeType,
            }
        }
    }

    impl From<rpc::FilterType> for FilterType {
        fn from(filter_type: rpc::FilterType) -> Self {
            match filter_type {
                rpc::FilterType::GlobPattern => Self::GlobPattern,
                rpc::FilterType::MimeType => Self::MimeType,
            }
        }
    }

    #[derive(Serialize, Deserialize, Type, Clone, Debug)]
    /// Presents the user with a choice to select from or as a checkbox.
    pub struct Choice(String, String, Vec<(String, String)>, String);

    impl From<Choice> for rpc::Choice {
        fn from(val: Choice) -> Self {
            let mut choices = HashMap::new();
            for (key, value) in val.2 {
                choices.insert(key, value);
            }
            rpc::Choice {
                label: val.1,
                choices,
                initial_selection: val.3,
            }
        }
    }

    #[derive(DeserializeDict, SerializeDict, TypeDict, Clone, Debug, Default)]
    pub struct OpenFileOptions {
        accept_label: Option<String>,
        modal: Option<bool>,
        multiple: Option<bool>,
        directory: Option<bool>,
        filters: Option<Vec<FileFilter>>,
        current_filter: Option<FileFilter>,
        choices: Option<Vec<Choice>>,
    }

    impl From<OpenFileOptions> for rpc::OpenFileOptions {
        fn from(val: OpenFileOptions) -> Self {
            let mut filters = Vec::new();
            for f in val.filters.unwrap_or_default() {
                filters.push(f.into());
            }

            let mut choices = HashMap::new();
            for c in val.choices.unwrap_or_default() {
                choices.insert(c.0.clone(), c.into());
            }

            rpc::OpenFileOptions {
                accept_label: val.accept_label,
                modal: val.modal,
                multiple: val.multiple,
                directory: val.directory,
                filters,
                current_filter: val.current_filter.map(|f| f.into()),
                choices,
            }
        }
    }

    #[derive(DeserializeDict, SerializeDict, TypeDict, Clone, Debug, Default)]
    pub struct OpenFileResults {
        uris: Vec<String>,
        choices: Vec<(String, String)>,
        current_filter: Option<FileFilter>,
        writable: Option<bool>,
    }

    impl From<rpc::OpenFileResults> for OpenFileResults {
        fn from(results: rpc::OpenFileResults) -> Self {
            let mut choices = Vec::new();
            for c in results.choices {
                choices.push(c);
            }

            Self {
                uris: results.uris,
                choices,
                current_filter: results.current_filter.map(FileFilter::from),
                writable: results.writable,
            }
        }
    }

    #[derive(DeserializeDict, SerializeDict, TypeDict, Clone, Debug, Default)]
    pub struct SaveFileOptions {
        accept_label: Option<String>,
        modal: Option<bool>,
        multiple: Option<bool>,
        filters: Option<Vec<FileFilter>>,
        current_filter: Option<FileFilter>,
        choices: Option<Vec<Choice>>,
        current_name: Option<String>,
        current_folder: Option<Vec<u8>>,
        current_file: Option<Vec<u8>>,
    }

    impl From<SaveFileOptions> for rpc::SaveFileOptions {
        fn from(options: SaveFileOptions) -> Self {
            let mut filters = Vec::new();
            for f in options.filters.unwrap_or_default() {
                filters.push(f.into());
            }

            let mut choices = HashMap::new();
            for c in options.choices.unwrap_or_default() {
                choices.insert(c.0.clone(), c.into());
            }

            Self {
                accept_label: options.accept_label,
                modal: options.modal,
                filters,
                current_filter: options.current_filter.map(|f| f.into()),
                choices,
                current_name: options.current_name,
                current_folder: options
                    .current_folder
                    .map(|f| String::from_utf8_lossy(&f).into_owned()),
                current_file: options
                    .current_file
                    .map(|f| String::from_utf8_lossy(&f).into_owned()),
            }
        }
    }

    #[derive(DeserializeDict, SerializeDict, TypeDict, Clone, Debug, Default)]
    pub struct SaveFileResults {
        uris: Vec<String>,
        choices: Vec<(String, String)>,
        current_filter: Option<FileFilter>,
    }

    impl From<rpc::SaveFileResults> for SaveFileResults {
        fn from(results: rpc::SaveFileResults) -> Self {
            let mut choices = Vec::new();
            for c in results.choices {
                choices.push(c);
            }

            Self {
                uris: results.uris,
                choices,
                current_filter: results.current_filter.map(FileFilter::from),
            }
        }
    }

    #[derive(DeserializeDict, SerializeDict, TypeDict, Clone, Debug, Default)]
    pub struct SaveFilesOptions {
        handle_token: Option<String>,
        accept_label: Option<String>,
        modal: Option<bool>,
        choices: Vec<Choice>,
        current_folder: Vec<u8>,
        current_file: Vec<Vec<u8>>,
    }

    #[derive(DeserializeDict, SerializeDict, TypeDict, Clone, Debug, Default)]
    pub struct SaveFilesResults {
        uris: Vec<String>,
        choices: Vec<(String, String)>,
    }
}
