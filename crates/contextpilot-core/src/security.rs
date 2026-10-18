//! Owner-only storage. Windows uses a protected DACL rather than inherited directory grants.
use anyhow::Result;
use std::path::Path;
pub fn owner_only(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            path,
            std::fs::Permissions::from_mode(if path.is_dir() { 0o700 } else { 0o600 }),
        )?;
    }
    #[cfg(windows)]
    windows::protect(path)?;
    Ok(())
}
#[cfg(windows)]
mod windows {
    use super::*;
    use anyhow::{anyhow, bail};
    use std::{ffi::c_void, os::windows::ffi::OsStrExt, ptr};
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn ConvertStringSecurityDescriptorToSecurityDescriptorW(
            text: *const u16,
            revision: u32,
            descriptor: *mut *mut c_void,
            size: *mut u32,
        ) -> i32;
        fn GetSecurityDescriptorDacl(
            descriptor: *const c_void,
            present: *mut i32,
            dacl: *mut *mut c_void,
            defaulted: *mut i32,
        ) -> i32;
        fn SetNamedSecurityInfoW(
            name: *mut u16,
            object_type: i32,
            information: u32,
            owner: *mut c_void,
            group: *mut c_void,
            dacl: *mut c_void,
            sacl: *mut c_void,
        ) -> u32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LocalFree(memory: *mut c_void) -> *mut c_void;
    }
    fn user_sid() -> Result<String> {
        let root = std::env::var_os("SystemRoot")
            .ok_or_else(|| anyhow!("Windows system directory unavailable"))?;
        let result =
            std::process::Command::new(std::path::PathBuf::from(root).join("System32/whoami.exe"))
                .args(["/user", "/fo", "csv", "/nh"])
                .output()?;
        if !result.status.success() {
            bail!("Cannot identify Windows user for private storage");
        }
        let output = String::from_utf8(result.stdout)?;
        output
            .split('"')
            .find(|s| {
                s.starts_with("S-1-")
                    && s.bytes()
                        .all(|b| b.is_ascii_digit() || b == b'S' || b == b'-')
            })
            .map(str::to_owned)
            .ok_or_else(|| anyhow!("Cannot identify Windows SID"))
    }
    pub(super) fn protect(path: &Path) -> Result<()> {
        let sid = user_sid()?;
        let sddl: Vec<u16> = format!(
            "D:P(A;{};FA;;;{sid})",
            if path.is_dir() { "OICI" } else { "" }
        )
        .encode_utf16()
        .chain(Some(0))
        .collect();
        let mut descriptor = ptr::null_mut();
        let mut dacl = ptr::null_mut();
        let mut present = 0;
        let mut defaulted = 0;
        let mut name: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        // APIs documented by Microsoft: allocated descriptor owns the DACL until LocalFree.
        // No ACL pointers escape this scope. Protected DACL removes inherited public access.
        let status = unsafe {
            if ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                1,
                &mut descriptor,
                ptr::null_mut(),
            ) == 0
            {
                return Err(anyhow!("Cannot construct private Windows permissions"));
            }
            let valid =
                GetSecurityDescriptorDacl(descriptor, &mut present, &mut dacl, &mut defaulted) != 0
                    && present != 0
                    && !dacl.is_null();
            let status = if valid {
                SetNamedSecurityInfoW(
                    name.as_mut_ptr(),
                    1,
                    0x80000004,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    dacl,
                    ptr::null_mut(),
                )
            } else {
                1
            };
            LocalFree(descriptor);
            status
        };
        if status != 0 {
            bail!("Cannot protect local file with owner-only Windows permissions");
        }
        Ok(())
    }
}
