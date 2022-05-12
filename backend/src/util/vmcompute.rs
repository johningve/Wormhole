// Copyright (c) 2022 John Ingve Olsen
//
// This software is released under the MIT License.
// https://opensource.org/licenses/MIT

use uuid::Uuid;

mod hcs {

    use scopeguard::defer;
    use serde::Deserialize;
    use uuid::Uuid;
    use widestring::{WideCStr, WideCString};
    use windows::Win32::System::{
        Com::CoTaskMemFree,
        LibraryLoader::{FreeLibrary, GetProcAddress, LoadLibraryA},
    };

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    pub struct ComputeSystem {
        pub id: Uuid,
        pub state: String,
        pub system_type: String,
        pub owner: String,
        pub runtime_id: Uuid,
    }

    pub fn enumerate_compute_systems(query: &str) -> std::io::Result<Vec<ComputeSystem>> {
        let module = unsafe { LoadLibraryA("vmcompute.dll") };
        if module.is_invalid() {
            return Err(std::io::Error::last_os_error());
        }
        defer! {
            unsafe { FreeLibrary(module) };
        }

        let func = unsafe { GetProcAddress(module, "HcsEnumerateComputeSystems") }.unwrap();
        let func: unsafe extern "system" fn(
            query: *const u16,
            compute_systems: *mut *mut u16,
            result: *mut *mut u16,
        ) -> i32 = unsafe { std::mem::transmute(func) }; // VERY unsafe

        let query_wide = WideCString::from_str(query).unwrap();
        let mut compute_systems: *mut u16 = std::ptr::null_mut();
        let mut result: *mut u16 = std::ptr::null_mut();

        let hr = unsafe { func(query_wide.into_raw(), &mut compute_systems, &mut result) };
        if hr != 0 {
            return Err(std::io::Error::from_raw_os_error(hr));
        }
        defer! {
            unsafe { CoTaskMemFree(compute_systems as _) };
            unsafe { CoTaskMemFree(result as _) };
        }

        let compute_systems = if compute_systems.is_null() {
            String::new()
        } else {
            unsafe { WideCStr::from_ptr_str(compute_systems.as_ref().unwrap()) }.to_string_lossy()
        };

        serde_json::from_str(&compute_systems)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))
    }
}

pub fn get_wsl_vmid() -> std::io::Result<Uuid> {
    let vms = hcs::enumerate_compute_systems("{}")?;
    for vm in vms {
        if vm.owner == "WSL" {
            return Ok(vm.id);
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "WSL VM Not found",
    ))
}
