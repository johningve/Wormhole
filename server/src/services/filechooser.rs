use std::{
    collections::HashMap,
    ffi::OsStr,
    fs,
    mem::MaybeUninit,
    path::{Path, PathBuf},
};

use bindings::Windows::Win32::{
    Foundation::PWSTR,
    System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_ALL},
    UI::Shell::{
        IFileDialogCustomize, IFileOpenDialog, IFileSaveDialog, IShellItem,
        SHCreateItemFromParsingName, COMDLG_FILTERSPEC, FOS_ALLOWMULTISELECT, FOS_PICKFOLDERS,
        SIGDN_FILESYSPATH, _FILEOPENDIALOGOPTIONS,
    },
};
use regex::Regex;
use rpc::filechooser::{
    file_chooser_server::FileChooser, Choice, FileFilter, OpenFileRequest, OpenFileResults,
    SaveFileRequest, SaveFileResults, SaveFilesRequest, SaveFilesResults,
};
use widestring::WideCStr;
use windows::Guid;
use windows::Interface;

use crate::{util, wslpath};

// TODO: replace when windows-rs provides these instead.
const CLSID_FILE_OPEN_DIALOG: &str = "DC1C5A9C-E88A-4dde-A5A1-60F82A20AEF7";
const CLSID_FILE_SAVE_DIALOG: &str = "C0B4E2F3-BA21-4773-8DBA-335EC946EB8B";

const IID_SHELL_ITEM: &str = "43826D1E-E718-42EE-BC55-A1E261C37BFE";

#[derive(Default)]
pub struct FileChooserService {}

impl FileChooserService {
    fn open_file_sync(distro: &str, request: OpenFileRequest) -> anyhow::Result<OpenFileResults> {
        let options = request.options.unwrap_or_default();

        let dialog: IFileOpenDialog =
            unsafe { CoCreateInstance(&Guid::from(CLSID_FILE_OPEN_DIALOG), None, CLSCTX_ALL) }?;

        unsafe { dialog.SetTitle(request.title) }?;

        if let Some(accept_label) = &options.accept_label {
            unsafe { dialog.SetOkButtonLabel(accept_label.as_str()) }?;
        }

        let mut dialog_options = _FILEOPENDIALOGOPTIONS(unsafe { dialog.GetOptions() }? as _);

        if options.multiple() {
            dialog_options.0 |= FOS_ALLOWMULTISELECT.0;
        }

        if options.directory() {
            dialog_options.0 |= FOS_PICKFOLDERS.0;
        }

        unsafe { dialog.SetOptions(dialog_options.0 as _) }?;

        // file_types must not be dropped before the dialog itself is dropped.
        let file_types = FileTypes::from(&options.filters);
        let (count, ptr) = file_types.get_ptr();
        // SAFETY: we ensure that dialog is dropped before the file_types structure which holds the data.
        unsafe { dialog.SetFileTypes(count, ptr) }?;

        let id_mapping = Self::add_choices(dialog.cast()?, &options.choices)?;

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
            uris.push(String::from("file://") + &wslpath::to_wsl(distro, Path::new(&path))?);
        }

        let choices = Self::read_choices(dialog.cast()?, &options.choices, &id_mapping)?;

        let current_filter = {
            let file_type_index = unsafe { dialog.GetFileTypeIndex() }? as usize;
            if file_type_index < file_types.indices.len() {
                let filter_index = file_types.indices[file_type_index];
                Some(options.filters[filter_index].clone())
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

    fn save_file_sync(distro: &str, request: SaveFileRequest) -> anyhow::Result<SaveFileResults> {
        let options = request.options.unwrap_or_default();

        let dialog: IFileSaveDialog =
            unsafe { CoCreateInstance(&Guid::from(CLSID_FILE_SAVE_DIALOG), None, CLSCTX_ALL) }?;

        unsafe { dialog.SetTitle(request.title) }?;

        if let Some(accept_label) = &options.accept_label {
            unsafe { dialog.SetOkButtonLabel(accept_label.as_str()) }?;
        }

        if let Some(file_name) = &options.current_name {
            unsafe { dialog.SetFileName(file_name.as_str()) }?;
        }

        let folder_item: Option<IShellItem>;

        if let Some(folder) = &options.current_folder {
            unsafe {
                let mut item: MaybeUninit<IShellItem> = MaybeUninit::uninit();
                SHCreateItemFromParsingName(
                    wslpath::to_windows(distro, folder).as_os_str(),
                    None,
                    &Guid::from(IID_SHELL_ITEM),
                    item.as_mut_ptr() as _,
                )?;
                folder_item = Some(item.assume_init());
                dialog.SetFolder(folder_item.unwrap())?;
            }
        }

        let file_item: Option<IShellItem>;

        if let Some(file) = &options.current_file {
            unsafe {
                let mut item: MaybeUninit<IShellItem> = MaybeUninit::uninit();
                SHCreateItemFromParsingName(
                    wslpath::to_windows(distro, file).as_os_str(),
                    None,
                    &Guid::from(IID_SHELL_ITEM),
                    item.as_mut_ptr() as _,
                )?;
                file_item = Some(item.assume_init());
                dialog.SetSaveAsItem(file_item.unwrap())?;
            }
        }

        // file_types must not be dropped before the dialog itself is dropped.
        let file_types = FileTypes::from(&options.filters);
        let (count, ptr) = file_types.get_ptr();
        // SAFETY: we ensure that dialog is dropped before the file_types structure which holds the data.
        unsafe { dialog.SetFileTypes(count, ptr) }?;

        let id_mapping = Self::add_choices(dialog.cast()?, &options.choices)?;

        let item = unsafe {
            dialog.Show(None)?;
            dialog.GetResult()?
        };

        let path_raw = unsafe { item.GetDisplayName(SIGDN_FILESYSPATH) }?;
        let path = unsafe { WideCStr::from_ptr_str(path_raw.0) }.to_os_string();
        unsafe { CoTaskMemFree(path_raw.0 as _) };
        let uri = String::from("file://") + &wslpath::to_wsl(distro, Path::new(&path))?;

        let choices = Self::read_choices(dialog.cast()?, &options.choices, &id_mapping)?;

        let current_filter = {
            let file_type_index = unsafe { dialog.GetFileTypeIndex() }? as usize;
            if file_type_index < file_types.indices.len() {
                let filter_index = file_types.indices[file_type_index];
                Some(options.filters[filter_index].clone())
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
        distro: &str,
        request: SaveFilesRequest,
    ) -> anyhow::Result<SaveFilesResults> {
        let options = request.options.unwrap_or_default();

        let dialog: IFileOpenDialog =
            unsafe { CoCreateInstance(&Guid::from(CLSID_FILE_OPEN_DIALOG), None, CLSCTX_ALL) }?;

        unsafe { dialog.SetTitle(request.title) }?;

        if let Some(accept_label) = &options.accept_label {
            unsafe { dialog.SetOkButtonLabel(accept_label.as_str()) }?;
        }

        let folder_item: Option<IShellItem>;

        if let Some(folder) = &options.current_folder {
            unsafe {
                let mut item: MaybeUninit<IShellItem> = MaybeUninit::uninit();
                SHCreateItemFromParsingName(
                    wslpath::to_windows(distro, folder).as_os_str(),
                    None,
                    &Guid::from(IID_SHELL_ITEM),
                    item.as_mut_ptr() as _,
                )?;
                folder_item = Some(item.assume_init());
                dialog.SetFolder(folder_item.unwrap())?;
            }
        }

        let id_mapping = Self::add_choices(dialog.cast()?, &options.choices)?;

        let folder_item = unsafe {
            dialog.Show(None)?;
            dialog.GetResult()?
        };

        let path_raw = unsafe { folder_item.GetDisplayName(SIGDN_FILESYSPATH) }?;
        let path = unsafe { WideCStr::from_ptr_str(path_raw.0) }.to_os_string();
        unsafe { CoTaskMemFree(path_raw.0 as _) };

        let mut uris = Vec::new();

        for name in &options.files {
            let full_path = Path::new(&path).join(name);
            if full_path.exists() {
                todo!()
            }

            uris.push(String::from("file://") + &wslpath::to_wsl(distro, &full_path)?);
        }

        let choices = Self::read_choices(dialog.cast()?, &options.choices, &id_mapping)?;

        Ok(SaveFilesResults { uris, choices })
    }

    fn add_choices(
        dialog: IFileDialogCustomize,
        choices: &'_ HashMap<String, Choice>,
    ) -> windows::Result<HashMap<u32, &'_ str>> {
        let mut id_mapping = HashMap::new();
        let mut id = 0;

        for (choice_id, choice) in choices {
            id_mapping.insert(id, choice_id.as_str());
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
        choices: &HashMap<String, Choice>,
        id_mapping: &HashMap<u32, &'_ str>,
    ) -> windows::Result<HashMap<String, String>> {
        let mut choice_results = HashMap::new();

        for (id, choice_id) in id_mapping {
            if let Some(choice) = choices.get(*choice_id) {
                if choice.choices.is_empty() {
                    let state = unsafe { dialog.GetCheckButtonState(*id) }?;
                    choice_results.insert(choice_id.to_string(), state.as_bool().to_string());
                } else {
                    let state = unsafe { dialog.GetSelectedControlItem(*id) }?;
                    if let Some(item_id) = id_mapping.get(&state) {
                        choice_results.insert(choice_id.to_string(), item_id.to_string());
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
            for filter_entry in &filter.entries {
                match filter_entry.r#type() {
                    rpc::filechooser::FilterType::GlobPattern => {
                        if !filter_spec.is_empty() {
                            filter_spec.push(';');
                        }
                        filter_spec.push_str(&glob_patter_to_filter(&filter_entry.filter));
                    }
                    rpc::filechooser::FilterType::MimeType => {
                        if let Some(extensions) =
                            new_mime_guess::get_mime_extensions_str(&filter_entry.filter)
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

#[tonic::async_trait]
impl FileChooser for FileChooserService {
    async fn open_file(
        &self,
        request: tonic::Request<OpenFileRequest>,
    ) -> Result<tonic::Response<OpenFileResults>, tonic::Status> {
        let distro_name = util::get_distro_name(&request)?;
        let request = request.into_inner();

        tokio::task::spawn_blocking(move || Self::open_file_sync(&distro_name, request))
            .await
            .map_err(util::map_to_status)
            .and_then(|r| r.map_err(util::map_to_status))
            .map(tonic::Response::new)
    }

    async fn save_file(
        &self,
        request: tonic::Request<SaveFileRequest>,
    ) -> Result<tonic::Response<SaveFileResults>, tonic::Status> {
        let distro_name = util::get_distro_name(&request)?;
        let request = request.into_inner();

        tokio::task::spawn_blocking(move || Self::save_file_sync(&distro_name, request))
            .await
            .map_err(util::map_to_status)
            .and_then(|r| r.map_err(util::map_to_status))
            .map(tonic::Response::new)
    }

    async fn save_files(
        &self,
        request: tonic::Request<SaveFilesRequest>,
    ) -> Result<tonic::Response<SaveFilesResults>, tonic::Status> {
        let distro_name = util::get_distro_name(&request)?;
        let request = request.into_inner();

        tokio::task::spawn_blocking(move || Self::save_files_sync(&distro_name, request))
            .await
            .map_err(util::map_to_status)
            .and_then(|r| r.map_err(util::map_to_status))
            .map(tonic::Response::new)
    }
}

#[cfg(test)]
mod tests {
    use bindings::Windows::Win32::{
        System::Com::{CoInitializeEx, COINIT_MULTITHREADED},
        UI::HiDpi::{SetProcessDpiAwareness, PROCESS_PER_MONITOR_DPI_AWARE},
    };

    #[test]
    fn test_get_open_file_name() {
        unsafe { SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE) }.unwrap();
        unsafe { CoInitializeEx(std::ptr::null_mut(), COINIT_MULTITHREADED) }.unwrap();
        println!("hello");
    }
}
