use std::{collections::HashMap, path::Path};

use regex::Regex;
use widestring::WideCStr;
use windows::core::Interface;
use windows::core::GUID;
use windows::Win32::{
    Foundation::PWSTR,
    System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_ALL},
    UI::Shell::{
        Common::COMDLG_FILTERSPEC, IFileDialogCustomize, IFileOpenDialog, IFileSaveDialog,
        IShellItem, SHCreateItemFromParsingName, FOS_ALLOWMULTISELECT, FOS_PICKFOLDERS,
        SIGDN_FILESYSPATH, _FILEOPENDIALOGOPTIONS,
    },
};

use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use zbus::{dbus_interface, Connection};
use zvariant::OwnedObjectPath;
use zvariant_derive::{DeserializeDict, SerializeDict, Type};

use crate::util::wslpath;

// TODO: replace when windows-rs provides these instead.
const CLSID_FILE_OPEN_DIALOG: &str = "DC1C5A9C-E88A-4dde-A5A1-60F82A20AEF7";
const CLSID_FILE_SAVE_DIALOG: &str = "C0B4E2F3-BA21-4773-8DBA-335EC946EB8B";

#[derive(Default, Clone)]
pub struct FileChooser {}

impl FileChooser {
    pub async fn init(connection: &Connection) -> zbus::Result<()> {
        connection
            .object_server()
            .at(super::PORTAL_PATH, FileChooser {})
            .await?;

        log::info!("FileChooser portal enabled.");

        Ok(())
    }

    fn open_file_sync(
        &self,
        _handle: OwnedObjectPath,
        _app_id: &str,
        _parent_window: &str,
        title: &str,
        options: OpenFileOptions,
    ) -> anyhow::Result<OpenFileResults> {
        let dialog: IFileOpenDialog =
            unsafe { CoCreateInstance(&GUID::from(CLSID_FILE_OPEN_DIALOG), None, CLSCTX_ALL) }?;

        unsafe { dialog.SetTitle(title) }?;

        if let Some(accept_label) = &options.accept_label {
            unsafe { dialog.SetOkButtonLabel(accept_label.as_str()) }?;
        }

        let mut dialog_options = unsafe { dialog.GetOptions() }? as _FILEOPENDIALOGOPTIONS;
        if options.multiple.unwrap_or_default() {
            dialog_options |= FOS_ALLOWMULTISELECT;
        }
        if options.directory.unwrap_or_default() {
            dialog_options |= FOS_PICKFOLDERS;
        }
        unsafe { dialog.SetOptions(dialog_options as _) }?;

        let filters = options.filters.unwrap_or_default();
        // file_types must not be dropped before the dialog itself is dropped.
        let file_types = FileTypes::from(&filters);
        let (count, ptr) = file_types.get_ptr();
        // SAFETY: we ensure that dialog is dropped before the file_types structure which holds the data.
        unsafe { dialog.SetFileTypes(count, ptr) }?;

        let choices = options.choices.unwrap_or_default();
        let id_mapping = Self::add_choices(dialog.cast()?, &choices)?;

        let dialog_results = unsafe {
            dialog.Show(None)?;
            dialog.GetResults()?
        };

        let mut uris = Vec::new();

        for i in 0..unsafe { dialog_results.GetCount() }? {
            let item = unsafe { dialog_results.GetItemAt(i) }?;
            let path_raw = unsafe { item.GetDisplayName(SIGDN_FILESYSPATH) }?;
            let path = unsafe { WideCStr::from_ptr_str(path_raw.0) }.to_os_string();
            unsafe { CoTaskMemFree(path_raw.0 as _) };
            uris.push(String::from("file://") + &wslpath::to_wsl(Path::new(&path))?);
        }

        let choices = Self::read_choices(dialog.cast()?, &choices, &id_mapping)?;

        let current_filter = {
            let file_type_index = unsafe { dialog.GetFileTypeIndex() }? as usize;
            if file_type_index < file_types.indices.len() {
                let filter_index = file_types.indices[file_type_index];
                Some(filters[filter_index].clone())
            } else {
                None
            }
        };

        drop(dialog);

        Ok(OpenFileResults {
            uris,
            choices,
            current_filter,
            writable: Some(true),
        })
    }

    fn save_file_sync(
        &self,
        _handle: OwnedObjectPath,
        _app_id: &str,
        _parent_window: &str,
        title: &str,
        options: SaveFileOptions,
    ) -> anyhow::Result<SaveFileResults> {
        let dialog: IFileSaveDialog =
            unsafe { CoCreateInstance(&GUID::from(CLSID_FILE_SAVE_DIALOG), None, CLSCTX_ALL) }?;

        unsafe { dialog.SetTitle(title) }?;

        if let Some(accept_label) = &options.accept_label {
            unsafe { dialog.SetOkButtonLabel(accept_label.as_str()) }?;
        }

        if let Some(file_name) = &options.current_name {
            unsafe { dialog.SetFileName(file_name.as_str()) }?;
        }

        let folder_item: Option<IShellItem>;

        if let Some(folder) = options.current_folder {
            unsafe {
                let item: IShellItem = SHCreateItemFromParsingName(
                    wslpath::to_windows(&String::from_utf8(folder)?).as_os_str(),
                    None,
                )?;
                folder_item = Some(item);
                dialog.SetFolder(folder_item.unwrap())?;
            }
        }

        let file_item: Option<IShellItem>;

        if let Some(file) = options.current_file {
            unsafe {
                let item: IShellItem = SHCreateItemFromParsingName(
                    wslpath::to_windows(&String::from_utf8(file)?).as_os_str(),
                    None,
                )?;
                file_item = Some(item);
                dialog.SetSaveAsItem(file_item.unwrap())?;
            }
        }

        let filters = options.filters.unwrap_or_default();
        // file_types must not be dropped before the dialog itself is dropped.
        let file_types = FileTypes::from(&filters);
        let (count, ptr) = file_types.get_ptr();
        // SAFETY: we ensure that dialog is dropped before the file_types structure which holds the data.
        unsafe { dialog.SetFileTypes(count, ptr) }?;

        let choices = options.choices.unwrap_or_default();
        let id_mapping = Self::add_choices(dialog.cast()?, &choices)?;

        let item = unsafe {
            dialog.Show(None)?;
            dialog.GetResult()?
        };

        let path_raw = unsafe { item.GetDisplayName(SIGDN_FILESYSPATH) }?;
        let path = unsafe { WideCStr::from_ptr_str(path_raw.0) }.to_os_string();
        unsafe { CoTaskMemFree(path_raw.0 as _) };
        let uri = String::from("file://") + &wslpath::to_wsl(Path::new(&path))?;

        let choices = Self::read_choices(dialog.cast()?, &choices, &id_mapping)?;

        let current_filter = {
            let file_type_index = unsafe { dialog.GetFileTypeIndex() }? as usize;
            if file_type_index < file_types.indices.len() {
                let filter_index = file_types.indices[file_type_index];
                Some(filters[filter_index].clone())
            } else {
                None
            }
        };

        drop(dialog);

        Ok(SaveFileResults {
            uris: vec![uri],
            choices,
            current_filter,
        })
    }

    fn save_files_sync(
        &self,
        _handle: OwnedObjectPath,
        _app_id: &str,
        _parent_window: &str,
        title: &str,
        options: SaveFilesOptions,
    ) -> anyhow::Result<SaveFilesResults> {
        let dialog: IFileOpenDialog =
            unsafe { CoCreateInstance(&GUID::from(CLSID_FILE_OPEN_DIALOG), None, CLSCTX_ALL) }?;

        unsafe { dialog.SetTitle(title) }?;

        if let Some(accept_label) = &options.accept_label {
            unsafe { dialog.SetOkButtonLabel(accept_label.as_str()) }?;
        }

        let folder_item: Option<IShellItem>;

        if let Some(folder) = options.current_folder {
            unsafe {
                let item: IShellItem = SHCreateItemFromParsingName(
                    wslpath::to_windows(&String::from_utf8(folder)?).as_os_str(),
                    None,
                )?;
                folder_item = Some(item);
                dialog.SetFolder(folder_item.unwrap())?;
            }
        }

        let choices = options.choices.unwrap_or_default();
        let id_mapping = Self::add_choices(dialog.cast()?, &choices)?;

        let folder_item = unsafe {
            dialog.Show(None)?;
            dialog.GetResult()?
        };

        let path_raw = unsafe { folder_item.GetDisplayName(SIGDN_FILESYSPATH) }?;
        let path = unsafe { WideCStr::from_ptr_str(path_raw.0) }.to_os_string();
        unsafe { CoTaskMemFree(path_raw.0 as _) };

        let mut uris = Vec::new();

        for name in options.files.unwrap_or_default() {
            let full_path = Path::new(&path).join(String::from_utf8(name)?);
            if full_path.exists() {
                todo!()
            }

            uris.push(String::from("file://") + &wslpath::to_wsl(&full_path)?);
        }

        let choices = Self::read_choices(dialog.cast()?, &choices, &id_mapping)?;

        Ok(SaveFilesResults { uris, choices })
    }

    fn add_choices(
        dialog: IFileDialogCustomize,
        choices: &[Choice],
    ) -> windows::core::Result<HashMap<u32, &'_ str>> {
        let mut id_mapping = HashMap::new();
        let mut id = 0;

        for choice in choices {
            id_mapping.insert(id, choice.id.as_str());
            if choice.choices.is_empty() {
                unsafe {
                    dialog.AddCheckButton(
                        id,
                        choice.label.clone(),
                        choice.initial_selection == "true",
                    )
                }?;
            } else {
                unsafe { dialog.AddMenu(id, choice.label.as_str()) }?;
                for (item_id, item_label) in &choice.choices {
                    unsafe { dialog.AddControlItem(id, id + 1, item_label.as_str()) }?;
                    id_mapping.insert(id, item_id.as_str());
                    id += 1;
                }
            }
            id += 1;
        }

        Ok(id_mapping)
    }

    fn read_choices(
        dialog: IFileDialogCustomize,
        choices: &[Choice],
        id_mapping: &HashMap<u32, &'_ str>,
    ) -> windows::core::Result<Vec<(String, String)>> {
        let mut choice_results = Vec::new();

        for (id, choice_id) in id_mapping {
            if let Some(choice) = choices.iter().find(|c| c.id == *choice_id) {
                if choice.choices.is_empty() {
                    let state = unsafe { dialog.GetCheckButtonState(*id) }?;
                    choice_results.push((choice_id.to_string(), state.as_bool().to_string()));
                } else {
                    let state = unsafe { dialog.GetSelectedControlItem(*id) }?;
                    if let Some(item_id) = id_mapping.get(&state) {
                        choice_results.push((choice_id.to_string(), item_id.to_string()));
                    }
                }
            }
        }

        Ok(choice_results)
    }
}

fn glob_patter_to_filter(glob_pattern: &str) -> String {
    let re = Regex::new(r"\[(\p{alpha}{2})\]").unwrap();
    let mut filter = String::from(glob_pattern);

    // need to reverse, otherwise the indices will get messed up
    for capture in re
        .captures_iter(glob_pattern)
        .collect::<Vec<_>>() // need to collect to a vec before we can reverse
        .iter()
        .rev()
    {
        let mut chars = capture.get(1).unwrap().as_str().chars();
        let first = chars.next().unwrap().to_lowercase().to_string();
        let second = chars.next().unwrap().to_lowercase().to_string();
        if first == second {
            filter.replace_range(capture.get(0).unwrap().range(), &first);
        }
    }

    filter
}

struct FileTypes {
    // storage for the wide strings
    _wstrings: Vec<Vec<u16>>,
    file_types: Vec<COMDLG_FILTERSPEC>,
    indices: Vec<usize>,
}

impl From<&Vec<FileFilter>> for FileTypes {
    fn from(filters: &Vec<FileFilter>) -> Self {
        let mut wstrings = Vec::new();
        let mut file_types = Vec::new();
        let mut indices = Vec::new();

        for (i, filter) in filters.iter().enumerate() {
            let mut filter_spec = String::new();
            for filter_entry in &filter.filters {
                match filter_entry.0 {
                    FilterType::GlobPattern => {
                        if !filter_spec.is_empty() {
                            filter_spec.push(';');
                        }
                        filter_spec.push_str(&glob_patter_to_filter(&filter_entry.1));
                    }
                    FilterType::MimeType => {
                        if let Some(extensions) =
                            new_mime_guess::get_mime_extensions_str(&filter_entry.1)
                        {
                            for extension in extensions {
                                if !filter_spec.is_empty() {
                                    filter_spec.push(';');
                                }
                                filter_spec.push_str(&(String::from("*.") + *extension));
                            }
                        }
                    }
                }
            }

            if filter_spec.is_empty() {
                continue;
            }

            // convert label and spec to wstrings
            let mut label_wide = filter
                .label
                .encode_utf16()
                .chain([0u16])
                .collect::<Vec<u16>>();
            let mut filter_spec_wide = filter_spec
                .encode_utf16()
                .chain([0u16])
                .collect::<Vec<u16>>();

            file_types.push(COMDLG_FILTERSPEC {
                pszName: PWSTR(label_wide.as_mut_ptr()),
                pszSpec: PWSTR(filter_spec_wide.as_mut_ptr()),
            });

            // keep references to the vectors so that they don't get dropped.
            wstrings.push(label_wide);
            wstrings.push(filter_spec_wide);

            indices.push(i);
        }

        Self {
            _wstrings: wstrings,
            file_types,
            indices,
        }
    }
}

impl FileTypes {
    fn get_ptr(&self) -> (u32, *const COMDLG_FILTERSPEC) {
        (self.file_types.len() as _, self.file_types.as_ptr())
    }
}

#[dbus_interface(name = "org.freedesktop.impl.portal.FileChooser")]
impl FileChooser {
    async fn open_file(
        &mut self,
        handle: OwnedObjectPath,
        app_id: String,
        parent_window: String,
        title: String,
        options: OpenFileOptions,
    ) -> (u32, OpenFileResults) {
        log::debug!("open_file called: ");
        log::debug!("\thandle: {}", handle.as_str());
        log::debug!("\tapp_id: {}", app_id);
        log::debug!("\tparent_window: {}", parent_window);
        log::debug!("\ttitle: {}", title);
        log::debug!("\toptions: {:?}", options);

        let c = self.clone();

        match tokio::task::spawn_blocking(move || {
            c.open_file_sync(handle, &app_id, &parent_window, &title, options)
        })
        .await
        .map_err(|e| log::error!("open_file errored: {}", e))
        .and_then(|r| r.map_err(|e| log::error!("open_file errored: {}", e)))
        {
            Ok(r) => (0, r),
            Err(_) => (1, OpenFileResults::default()),
        }
    }

    async fn save_file(
        &mut self,
        handle: OwnedObjectPath,
        app_id: String,
        parent_window: String,
        title: String,
        options: SaveFileOptions,
    ) -> (u32, SaveFileResults) {
        log::debug!("save_file called: ");
        log::debug!("\thandle: {}", handle.as_str());
        log::debug!("\tapp_id: {}", app_id);
        log::debug!("\tparent_window: {}", parent_window);
        log::debug!("\ttitle: {}", title);
        log::debug!("\toptions: {:?}", options);

        let c = self.clone();

        match tokio::task::spawn_blocking(move || {
            c.save_file_sync(handle, &app_id, &parent_window, &title, options)
        })
        .await
        .map_err(|e| log::error!("save_file errored: {}", e))
        .and_then(|r| r.map_err(|e| log::error!("save_file errored: {}", e)))
        {
            Ok(r) => (0, r),
            Err(_) => (1, SaveFileResults::default()),
        }
    }

    async fn save_files(
        &mut self,
        handle: OwnedObjectPath,
        app_id: String,
        parent_window: String,
        title: String,
        options: SaveFilesOptions,
    ) -> (u32, SaveFilesResults) {
        log::debug!("save_files called: ");
        log::debug!("\thandle: {}", handle.as_str());
        log::debug!("\tapp_id: {}", app_id);
        log::debug!("\tparent_window: {}", parent_window);
        log::debug!("\ttitle: {}", title);
        log::debug!("\toptions: {:?}", options);

        let c = self.clone();

        match tokio::task::spawn_blocking(move || {
            c.save_files_sync(handle, &app_id, &parent_window, &title, options)
        })
        .await
        .map_err(|e| log::error!("save_files errored: {}", e))
        .and_then(|r| r.map_err(|e| log::error!("save_files errored: {}", e)))
        {
            Ok(r) => (0, r),
            Err(_) => (1, SaveFilesResults::default()),
        }
    }
}

#[derive(Serialize, Deserialize, Type, Clone, Debug)]
/// A file filter, to limit the available file choices to a mimetype or a glob
/// pattern.
pub struct FileFilter {
    label: String,
    filters: Vec<(FilterType, String)>,
}

#[derive(Serialize_repr, Clone, Deserialize_repr, PartialEq, Debug, Type)]
#[repr(u32)]
#[doc(hidden)]
enum FilterType {
    GlobPattern = 0,
    MimeType = 1,
}

#[derive(Serialize, Deserialize, Type, Clone, Debug)]
/// Presents the user with a choice to select from or as a checkbox.
pub struct Choice {
    id: String,
    label: String,
    choices: Vec<(String, String)>,
    initial_selection: String,
}

#[derive(DeserializeDict, SerializeDict, Type, Clone, Debug, Default)]
#[zvariant(signature = "dict")]
pub struct OpenFileOptions {
    accept_label: Option<String>,
    modal: Option<bool>,
    multiple: Option<bool>,
    directory: Option<bool>,
    filters: Option<Vec<FileFilter>>,
    current_filter: Option<FileFilter>,
    choices: Option<Vec<Choice>>,
}

#[derive(DeserializeDict, SerializeDict, Type, Clone, Debug, Default)]
#[zvariant(signature = "dict")]
pub struct OpenFileResults {
    uris: Vec<String>,
    choices: Vec<(String, String)>,
    current_filter: Option<FileFilter>,
    writable: Option<bool>,
}

#[derive(DeserializeDict, SerializeDict, Type, Clone, Debug, Default)]
#[zvariant(signature = "dict")]
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

#[derive(DeserializeDict, SerializeDict, Type, Clone, Debug, Default)]
#[zvariant(signature = "dict")]
pub struct SaveFileResults {
    uris: Vec<String>,
    choices: Vec<(String, String)>,
    current_filter: Option<FileFilter>,
}

#[derive(DeserializeDict, SerializeDict, Type, Clone, Debug, Default)]
#[zvariant(signature = "dict")]
pub struct SaveFilesOptions {
    handle_token: Option<String>,
    accept_label: Option<String>,
    modal: Option<bool>,
    choices: Option<Vec<Choice>>,
    current_folder: Option<Vec<u8>>,
    files: Option<Vec<Vec<u8>>>,
}

#[derive(DeserializeDict, SerializeDict, Type, Clone, Debug, Default)]
#[zvariant(signature = "dict")]
pub struct SaveFilesResults {
    uris: Vec<String>,
    choices: Vec<(String, String)>,
}
