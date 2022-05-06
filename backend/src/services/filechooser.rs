use std::collections::HashMap;
use std::convert::TryFrom;
use std::path::PathBuf;

use regex::Regex;
use scopeguard::defer;
use widestring::WideCStr;

use windows::core::Interface;
use windows::Win32::Foundation::PWSTR;
use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_INPROC_SERVER};
use windows::Win32::UI::{
    Shell::{
        Common::COMDLG_FILTERSPEC, FileOpenDialog, FileSaveDialog, IFileDialog,
        IFileDialogCustomize, IFileOpenDialog, IShellItem, FOS_ALLOWMULTISELECT, FOS_PICKFOLDERS,
        SIGDN_FILESYSPATH, _FILEOPENDIALOGOPTIONS,
    },
    WindowsAndMessaging::GetForegroundWindow,
};

use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use zbus::{dbus_interface, Connection};
use zvariant::{OwnedObjectPath, OwnedValue, Value};
use zvariant_derive::Type;

use crate::util::wslpath;

pub struct FileChooser;

enum DialogKind {
    OpenFile,
    SaveFile,
    SaveFiles,
}

impl FileChooser {
    pub async fn init(connection: &Connection) -> zbus::Result<()> {
        connection
            .object_server()
            .at(super::PORTAL_PATH, FileChooser {})
            .await?;

        log::info!("FileChooser portal enabled.");

        Ok(())
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
        options: HashMap<String, OwnedValue>,
    ) -> (u32, HashMap<String, OwnedValue>) {
        log::debug!("open_file called: ");
        log::debug!("\thandle: {}", handle.as_str());
        log::debug!("\tapp_id: {}", app_id);
        log::debug!("\tparent_window: {}", parent_window);
        log::debug!("\ttitle: {}", title);
        log::debug!("\toptions: {:?}", options);

        match tokio::task::spawn_blocking(move || {
            show_dialog(DialogKind::OpenFile {}, &title, options)
        })
        .await
        .map_err(|e| log::error!("open_file errored: {}", e))
        .and_then(|r| r.map_err(|e| log::error!("open_file errored: {}", e)))
        {
            Ok(r) => (0, r),
            Err(_) => (1, HashMap::new()),
        }
    }

    async fn save_file(
        &mut self,
        handle: OwnedObjectPath,
        app_id: String,
        parent_window: String,
        title: String,
        options: HashMap<String, OwnedValue>,
    ) -> (u32, HashMap<String, OwnedValue>) {
        log::debug!("save_file called: ");
        log::debug!("\thandle: {}", handle.as_str());
        log::debug!("\tapp_id: {}", app_id);
        log::debug!("\tparent_window: {}", parent_window);
        log::debug!("\ttitle: {}", title);
        log::debug!("\toptions: {:?}", options);

        match tokio::task::spawn_blocking(move || {
            show_dialog(DialogKind::SaveFile {}, &title, options)
        })
        .await
        .map_err(|e| log::error!("save_file errored: {}", e))
        .and_then(|r| r.map_err(|e| log::error!("save_file errored: {}", e)))
        {
            Ok(r) => (0, r),
            Err(_) => (1, HashMap::new()),
        }
    }

    async fn save_files(
        &mut self,
        handle: OwnedObjectPath,
        app_id: String,
        parent_window: String,
        title: String,
        options: HashMap<String, OwnedValue>,
    ) -> (u32, HashMap<String, OwnedValue>) {
        log::debug!("save_files called: ");
        log::debug!("\thandle: {}", handle.as_str());
        log::debug!("\tapp_id: {}", app_id);
        log::debug!("\tparent_window: {}", parent_window);
        log::debug!("\ttitle: {}", title);
        log::debug!("\toptions: {:?}", options);

        match tokio::task::spawn_blocking(move || {
            show_dialog(DialogKind::SaveFiles {}, &title, options)
        })
        .await
        .map_err(|e| log::error!("save_files errored: {}", e))
        .and_then(|r| r.map_err(|e| log::error!("save_files errored: {}", e)))
        {
            Ok(r) => (0, r),
            Err(_) => (1, HashMap::new()),
        }
    }
}

fn show_dialog(
    kind: DialogKind,
    title: &str,
    options: HashMap<String, OwnedValue>,
) -> anyhow::Result<HashMap<String, OwnedValue>> {
    let class_id = match kind {
        DialogKind::OpenFile => &FileOpenDialog,
        DialogKind::SaveFile => &FileSaveDialog,
        DialogKind::SaveFiles => &FileOpenDialog,
    };

    let dialog: IFileDialog = unsafe { CoCreateInstance(class_id, None, CLSCTX_INPROC_SERVER) }?;

    unsafe { dialog.SetTitle(title) }?;

    if let Some(label) = options.get("accept_label") {
        unsafe { dialog.SetOkButtonLabel(<&str>::try_from(label)?) }?;
    }

    let mut multi_select = false;
    if let Some(multiple) = options.get("multiple") {
        multi_select = bool::try_from(multiple)?;
    }

    let mut directory = false;
    if let Some(directory_value) = options.get("directory") {
        directory = bool::try_from(directory_value)?;
    }

    if matches!(kind, DialogKind::OpenFile | DialogKind::SaveFiles) {
        let mut dialog_options =
            unsafe { dialog.cast::<IFileOpenDialog>()?.GetOptions() }? as _FILEOPENDIALOGOPTIONS;

        if directory || matches!(kind, DialogKind::SaveFiles) {
            dialog_options |= FOS_PICKFOLDERS;
            directory = true;
        }

        if multi_select {
            dialog_options |= FOS_ALLOWMULTISELECT;
        }

        unsafe { dialog.SetOptions(dialog_options as _) }?;
    }

    let mut filters: Vec<FileFilter> = vec![];
    // file_types must not be dropped before the dialog itself is dropped.
    let mut file_types = FileTypes::default();
    if matches!(kind, DialogKind::OpenFile | DialogKind::SaveFile) {
        if let Some(filters_value) = options.get("filters") {
            filters = <Vec<FileFilter>>::try_from(filters_value.clone())?;
            file_types = FileTypes::from(filters.as_slice());
            let (count, ptr) = file_types.get_ptr();
            // SAFETY: we ensure that dialog is dropped before the file_types structure which holds the data.
            if matches!(kind, DialogKind::SaveFile) || !directory {
                unsafe { dialog.SetFileTypes(count, ptr) }?;
            }
        }
    }

    let mut choices: Vec<Choice> = vec![];
    let choices_id_mapping = if let Some(choices_value) = options.get("choices") {
        choices = <Vec<Choice>>::try_from(choices_value.clone())?;
        add_choices(dialog.cast()?, choices.as_slice())?
    } else {
        HashMap::new()
    };

    unsafe { dialog.Show(GetForegroundWindow())? };

    let mut results = HashMap::<String, OwnedValue>::new();

    let choices = read_choices(dialog.cast()?, &choices, &choices_id_mapping)?;
    results.insert(String::from("choices"), Value::try_from(choices)?.into());

    if !directory {
        let file_type_index = unsafe { dialog.GetFileTypeIndex() }? as usize;
        if file_type_index < file_types.indices.len() {
            let filter_index = file_types.indices[file_type_index];
            results.insert(
                String::from("current_filter"),
                Value::try_from(filters[filter_index].clone())?.into(),
            );
        }
    }

    let mut uris = Vec::new();

    match kind {
        DialogKind::OpenFile => {
            let dialog_results = unsafe { dialog.cast::<IFileOpenDialog>()?.GetResults()? };
            for i in 0..unsafe { dialog_results.GetCount() }? {
                let item = unsafe { dialog_results.GetItemAt(i) }?;
                uris.push(String::from("file://") + &wslpath::to_wsl(&get_path(&item)?)?);
            }
        }
        DialogKind::SaveFile => {
            let item = unsafe { dialog.GetResult() }?;
            let uri = String::from("file://") + &wslpath::to_wsl(&get_path(&item)?)?;
            uris.push(uri);
        }
        DialogKind::SaveFiles => {
            let item = unsafe { dialog.GetResult() }?;
            let path = get_path(&item)?;

            if let Some(files) = options.get("files") {
                for name in <Vec<Vec<u8>>>::try_from(files.clone())? {
                    let full_path = path.join(String::from_utf8(name)?);
                    if full_path.exists() {
                        todo!()
                    }
                    uris.push(String::from("file://") + &wslpath::to_wsl(&full_path)?);
                }
            }
        }
    }

    results.insert(String::from("uris"), Value::try_from(uris)?.into());

    Ok(results)
}

fn get_path(item: &IShellItem) -> windows::core::Result<PathBuf> {
    unsafe {
        let path_raw = item.GetDisplayName(SIGDN_FILESYSPATH)?;
        defer! { CoTaskMemFree(path_raw.0 as _) };
        Ok(PathBuf::from(
            // SAFETY: to_os_string() makes a copy of the string, so it is safe to free it afterwards
            WideCStr::from_ptr_str(path_raw.0).to_os_string(),
        ))
    }
}

fn add_choices(
    dialog: IFileDialogCustomize,
    choices: &[Choice],
) -> windows::core::Result<HashMap<u32, &'_ str>> {
    let mut id_mapping = HashMap::new();
    let mut id = 0;

    for choice in choices {
        id_mapping.insert(id, choice.id.as_str());
        if choice.selections.is_empty() {
            unsafe {
                dialog.AddCheckButton(id, choice.label.clone(), choice.initial_selection == "true")
            }?;
        } else {
            let menu_id = id;
            unsafe { dialog.AddMenu(menu_id, choice.label.as_str()) }?;
            for (item_id, item_label) in &choice.selections {
                id += 1;
                unsafe { dialog.AddControlItem(menu_id, id, item_label.as_str()) }?;
                if &choice.initial_selection == item_id {
                    unsafe { dialog.SetSelectedControlItem(menu_id, id) }?;
                }
                id_mapping.insert(id, item_id.as_str());
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
            if choice.selections.is_empty() {
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

#[derive(Default)]
struct FileTypes {
    // storage for the wide strings
    _wstrings: Vec<Vec<u16>>,
    file_types: Vec<COMDLG_FILTERSPEC>,
    indices: Vec<usize>,
}

impl From<&[FileFilter]> for FileTypes {
    fn from(filters: &[FileFilter]) -> Self {
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

#[derive(Serialize, Deserialize, Type, Clone, Debug, Value, OwnedValue)]
/// A file filter, to limit the available file choices to a mimetype or a glob
/// pattern.
pub struct FileFilter {
    label: String,
    filters: Vec<(FilterType, String)>,
}

#[derive(Serialize_repr, Clone, Deserialize_repr, PartialEq, Debug, Type, Value, OwnedValue)]
#[repr(u32)]
#[doc(hidden)]
enum FilterType {
    GlobPattern = 0,
    MimeType = 1,
}

#[derive(Serialize, Deserialize, Type, Clone, Debug, Value, OwnedValue)]
/// Presents the user with a choice to select from or as a checkbox.
pub struct Choice {
    id: String,
    label: String,
    selections: Vec<(String, String)>,
    initial_selection: String,
}
