use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CredentialError {
    StoreUnavailable,
    StoreFailed,
    InvalidSecret,
}

impl fmt::Display for CredentialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StoreUnavailable => write!(f, "OS credential store is unavailable"),
            Self::StoreFailed => write!(f, "OS credential store operation failed"),
            Self::InvalidSecret => write!(f, "stored credential is not valid UTF-8"),
        }
    }
}

impl std::error::Error for CredentialError {}

#[cfg(target_os = "linux")]
mod platform {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };

    use super::CredentialError;

    pub fn store(host: &str, username: &str, password: &str) -> Result<(), CredentialError> {
        let mut child = Command::new("secret-tool")
            .args([
                "store",
                "--label=XeRust360-RPC NOVA credential",
                "service",
                "xerust360-rpc",
                "host",
                host,
                "username",
                username,
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| CredentialError::StoreUnavailable)?;
        child
            .stdin
            .take()
            .ok_or(CredentialError::StoreFailed)?
            .write_all(password.as_bytes())
            .map_err(|_| CredentialError::StoreFailed)?;
        child
            .wait()
            .map_err(|_| CredentialError::StoreFailed)?
            .success()
            .then_some(())
            .ok_or(CredentialError::StoreFailed)
    }

    pub fn retrieve(host: &str, username: &str) -> Result<Option<String>, CredentialError> {
        let output = Command::new("secret-tool")
            .args([
                "lookup",
                "service",
                "xerust360-rpc",
                "host",
                host,
                "username",
                username,
            ])
            .output()
            .map_err(|_| CredentialError::StoreUnavailable)?;
        if !output.status.success() {
            return Ok(None);
        }
        let password =
            String::from_utf8(output.stdout).map_err(|_| CredentialError::InvalidSecret)?;
        Ok(Some(password.trim_end_matches(['\r', '\n']).to_owned()))
    }

    pub fn delete(host: &str, username: &str) -> Result<(), CredentialError> {
        let output = Command::new("secret-tool")
            .args([
                "clear",
                "service",
                "xerust360-rpc",
                "host",
                host,
                "username",
                username,
            ])
            .output()
            .map_err(|_| CredentialError::StoreUnavailable)?;
        output
            .status
            .success()
            .then_some(())
            .ok_or(CredentialError::StoreFailed)
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::{
        ffi::{CString, c_char, c_void},
        ptr, slice,
    };

    use super::CredentialError;

    #[link(name = "Security", kind = "framework")]
    unsafe extern "C" {
        fn SecKeychainFindGenericPassword(
            keychain: *mut c_void,
            service_length: u32,
            service: *const c_char,
            account_length: u32,
            account: *const c_char,
            password_length: *mut u32,
            password_data: *mut *mut c_void,
            item_ref: *mut *mut c_void,
        ) -> i32;
        fn SecKeychainAddGenericPassword(
            keychain: *mut c_void,
            service_length: u32,
            service: *const c_char,
            account_length: u32,
            account: *const c_char,
            password_length: u32,
            password_data: *const c_void,
            item_ref: *mut *mut c_void,
        ) -> i32;
        fn SecKeychainItemDelete(item_ref: *mut c_void) -> i32;
        fn SecKeychainItemFreeContent(attributes: *mut c_void, data: *mut c_void) -> i32;
    }

    fn cstrings(host: &str, username: &str) -> Result<(CString, CString), CredentialError> {
        let service = CString::new(format!("xerust360-rpc:{host}"))
            .map_err(|_| CredentialError::StoreFailed)?;
        let account = CString::new(username).map_err(|_| CredentialError::StoreFailed)?;
        Ok((service, account))
    }

    pub fn store(host: &str, username: &str, password: &str) -> Result<(), CredentialError> {
        let (service, account) = cstrings(host, username)?;
        let _ = delete(host, username);
        let mut item = ptr::null_mut();
        // SAFETY: input pointers reference live byte buffers for the duration of the API call.
        let status = unsafe {
            SecKeychainAddGenericPassword(
                ptr::null_mut(),
                service.as_bytes().len() as u32,
                service.as_ptr(),
                account.as_bytes().len() as u32,
                account.as_ptr(),
                password.len() as u32,
                password.as_ptr().cast(),
                &mut item,
            )
        };
        if status != 0 {
            return Err(CredentialError::StoreFailed);
        }
        Ok(())
    }

    pub fn retrieve(host: &str, username: &str) -> Result<Option<String>, CredentialError> {
        let (service, account) = cstrings(host, username)?;
        let mut length = 0;
        let mut data = ptr::null_mut();
        let mut item = ptr::null_mut();
        // SAFETY: output pointers are valid and initialized; input C strings remain alive.
        let status = unsafe {
            SecKeychainFindGenericPassword(
                ptr::null_mut(),
                service.as_bytes().len() as u32,
                service.as_ptr(),
                account.as_bytes().len() as u32,
                account.as_ptr(),
                &mut length,
                &mut data,
                &mut item,
            )
        };
        if status != 0 {
            return Ok(None);
        }
        // SAFETY: the keychain API returned `length` bytes at `data` on success.
        let bytes = unsafe { slice::from_raw_parts(data.cast::<u8>(), length as usize) };
        let password = String::from_utf8(bytes.to_vec());
        // SAFETY: memory was allocated by the keychain API and must be released with its matching function.
        let _ = unsafe { SecKeychainItemFreeContent(ptr::null_mut(), data) };
        Ok(Some(password.map_err(|_| CredentialError::InvalidSecret)?))
    }

    pub fn delete(host: &str, username: &str) -> Result<(), CredentialError> {
        let (service, account) = cstrings(host, username)?;
        let mut item = ptr::null_mut();
        // SAFETY: output pointer is valid and input C strings remain alive.
        let status = unsafe {
            SecKeychainFindGenericPassword(
                ptr::null_mut(),
                service.as_bytes().len() as u32,
                service.as_ptr(),
                account.as_bytes().len() as u32,
                account.as_ptr(),
                ptr::null_mut(),
                ptr::null_mut(),
                &mut item,
            )
        };
        if status != 0 || item.is_null() {
            return Ok(());
        }
        // SAFETY: `item` is a valid keychain item reference returned by the API.
        if unsafe { SecKeychainItemDelete(item) } == 0 {
            Ok(())
        } else {
            Err(CredentialError::StoreFailed)
        }
    }
}

#[cfg(windows)]
mod platform {
    use std::{ffi::c_void, ptr, slice};

    use super::CredentialError;

    #[repr(C)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    #[repr(C)]
    struct CredentialW {
        flags: u32,
        kind: u32,
        target_name: *mut u16,
        comment: *mut u16,
        last_written: FileTime,
        credential_blob_size: u32,
        credential_blob: *mut u8,
        persist: u32,
        attribute_count: u32,
        attributes: *mut c_void,
        target_alias: *mut u16,
        user_name: *mut u16,
    }

    #[link(name = "Advapi32")]
    unsafe extern "system" {
        fn CredWriteW(credential: *const CredentialW, flags: u32) -> i32;
        fn CredReadW(
            target: *const u16,
            kind: u32,
            flags: u32,
            credential: *mut *mut CredentialW,
        ) -> i32;
        fn CredDeleteW(target: *const u16, kind: u32, flags: u32) -> i32;
        fn CredFree(buffer: *mut c_void);
    }

    fn target(host: &str, username: &str) -> Vec<u16> {
        format!("XeRust360-RPC/{host}/{username}")
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect()
    }

    pub fn store(host: &str, username: &str, password: &str) -> Result<(), CredentialError> {
        let target_name = target(host, username);
        let mut blob = password.as_bytes().to_vec();
        let user = username
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let credential = CredentialW {
            flags: 0,
            kind: 1,
            target_name: target_name.as_ptr().cast_mut(),
            comment: ptr::null_mut(),
            last_written: FileTime { low: 0, high: 0 },
            credential_blob_size: blob.len() as u32,
            credential_blob: blob.as_mut_ptr(),
            persist: 2,
            attribute_count: 0,
            attributes: ptr::null_mut(),
            target_alias: ptr::null_mut(),
            user_name: user.as_ptr().cast_mut(),
        };
        // SAFETY: all pointers in the structure refer to live buffers for the duration of the call.
        let ok = unsafe { CredWriteW(&credential, 0) };
        (ok != 0).then_some(()).ok_or(CredentialError::StoreFailed)
    }

    pub fn retrieve(host: &str, username: &str) -> Result<Option<String>, CredentialError> {
        let target_name = target(host, username);
        let mut credential = ptr::null_mut();
        // SAFETY: `credential` is an output pointer; `target_name` is NUL terminated.
        let ok = unsafe { CredReadW(target_name.as_ptr(), 1, 0, &mut credential) };
        if ok == 0 || credential.is_null() {
            return Ok(None);
        }
        // SAFETY: the pointer is a valid structure returned by CredReadW until CredFree.
        let blob = unsafe { &*credential };
        // SAFETY: credential blob is valid for the reported size.
        let bytes = unsafe {
            slice::from_raw_parts(blob.credential_blob, blob.credential_blob_size as usize)
        };
        let password = String::from_utf8(bytes.to_vec());
        // SAFETY: releases the allocation returned by CredReadW.
        unsafe { CredFree(credential.cast()) };
        Ok(Some(password.map_err(|_| CredentialError::InvalidSecret)?))
    }

    pub fn delete(host: &str, username: &str) -> Result<(), CredentialError> {
        let target_name = target(host, username);
        // SAFETY: target name is NUL terminated and the function reads it synchronously.
        unsafe { CredDeleteW(target_name.as_ptr(), 1, 0) };
        Ok(())
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
mod platform {
    use super::CredentialError;

    pub fn store(_: &str, _: &str, _: &str) -> Result<(), CredentialError> {
        Err(CredentialError::StoreUnavailable)
    }
    pub fn retrieve(_: &str, _: &str) -> Result<Option<String>, CredentialError> {
        Err(CredentialError::StoreUnavailable)
    }
    pub fn delete(_: &str, _: &str) -> Result<(), CredentialError> {
        Err(CredentialError::StoreUnavailable)
    }
}

pub fn store(host: &str, username: &str, password: &str) -> Result<(), CredentialError> {
    platform::store(host, username, password)
}

pub fn retrieve(host: &str, username: &str) -> Result<Option<String>, CredentialError> {
    platform::retrieve(host, username)
}

pub fn delete(host: &str, username: &str) -> Result<(), CredentialError> {
    platform::delete(host, username)
}
