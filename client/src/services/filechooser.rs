use rpc::filechooser::{file_chooser_client::FileChooserClient, OpenFileRequest};
use tonic::{codegen::InterceptedService, transport::Channel};
use zbus::{dbus_interface, Connection};
use zvariant::OwnedObjectPath;

use super::interceptor::DistroInterceptor;

pub struct FileChooser {
    remote: FileChooserClient<InterceptedService<Channel, DistroInterceptor>>,
}

impl FileChooser {
    pub async fn init(grpc_channel: Channel, dbus_connection: &Connection) -> zbus::Result<()> {
        dbus_connection
            .request_name("org.freedesktop.impl.portal.FileChooser")
            .await?;

        dbus_connection.object_server_mut().await.at(
            super::PORTAL_PATH,
            FileChooser {
                remote: FileChooserClient::with_interceptor(grpc_channel, DistroInterceptor {}),
            },
        )?;

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
                (
                    result.response,
                    dbus::OpenFileResults::from(result.results.unwrap_or_default()),
                )
            }
            Err(e) => {
                log::error!("open_file errored: {}", e);
                (1, dbus::OpenFileResults::default())
            }
        }
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

    impl Into<rpc::FileFilter> for FileFilter {
        fn into(self) -> rpc::FileFilter {
            let mut entries = Vec::new();
            for (filter_type, filter) in self.1 {
                entries.push(rpc::file_filter::FilterEntry {
                    r#type: Into::<rpc::FilterType>::into(filter_type) as _,
                    filter,
                });
            }

            rpc::FileFilter {
                label: self.0,
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

    impl Into<rpc::FilterType> for FilterType {
        fn into(self) -> rpc::FilterType {
            match self {
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

    impl Into<rpc::Choice> for Choice {
        fn into(self) -> rpc::Choice {
            let mut choices = HashMap::new();
            for (key, value) in self.2 {
                choices.insert(key, value);
            }
            rpc::Choice {
                id: self.0,
                label: self.1,
                choices,
                initial_selection: self.3,
            }
        }
    }

    #[derive(DeserializeDict, SerializeDict, TypeDict, Clone, Debug, Default)]
    pub struct OpenFileOptions {
        accept_label: Option<String>,
        modal: Option<bool>,
        multiple: Option<bool>,
        directory: Option<bool>,
        filters: Vec<FileFilter>,
        current_filter: Option<FileFilter>,
        choices: Vec<Choice>,
    }

    impl Into<rpc::OpenFileOptions> for OpenFileOptions {
        fn into(self) -> rpc::OpenFileOptions {
            let mut filters = Vec::new();
            for f in self.filters {
                filters.push(f.into());
            }

            let mut choices = Vec::new();
            for c in self.choices {
                choices.push(c.into());
            }

            rpc::OpenFileOptions {
                accept_label: self.accept_label,
                modal: self.modal,
                multiple: self.multiple,
                directory: self.directory,
                filters,
                current_filter: self.current_filter.map(|f| f.into()),
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
                current_filter: results.current_filter.map(|f| FileFilter::from(f)),
                writable: results.writable,
            }
        }
    }
}
