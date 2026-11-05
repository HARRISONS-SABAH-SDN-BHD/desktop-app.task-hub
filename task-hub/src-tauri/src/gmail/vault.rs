//! Refresh tokens never enter workflow JSON, logs, receipts or frontend state.
use super::Credentials;
use crate::model::Result;

#[cfg(windows)]
mod platform {
    use super::*;
    use windows_sys::Win32::{
        Foundation::{GetLastError, ERROR_NOT_FOUND},
        Security::Credentials::*,
    };
    const TARGET: &str = "com.taskhub.desktop.gmail.oauth.v1";
    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(Some(0)).collect()
    }
    pub fn read() -> Result<Option<Credentials>> {
        let target = wide(TARGET);
        let mut pointer = std::ptr::null_mut();
        if unsafe { CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut pointer) } == 0 {
            return if unsafe { GetLastError() } == ERROR_NOT_FOUND {
                Ok(None)
            } else {
                Err("Windows Credential Manager could not read the Gmail connection".into())
            };
        }
        let credential = unsafe { &*pointer };
        let parsed = if credential.CredentialBlobSize == 0
            || credential.CredentialBlobSize > 5120
            || credential.CredentialBlob.is_null()
        {
            Err("Stored Gmail credentials are invalid. Disconnect and reconnect Gmail.".into())
        } else {
            let bytes = unsafe {
                std::slice::from_raw_parts(
                    credential.CredentialBlob,
                    credential.CredentialBlobSize as usize,
                )
            };
            serde_json::from_slice(bytes).map(Some).map_err(|_| {
                "Stored Gmail credentials are invalid. Disconnect and reconnect Gmail.".into()
            })
        };
        unsafe {
            CredFree(pointer.cast());
        }
        parsed
    }
    pub fn write(value: &Credentials) -> Result<()> {
        let mut bytes =
            serde_json::to_vec(value).map_err(|_| "Could not encode Gmail credentials")?;
        if bytes.len() > 5120 {
            return Err("Gmail credentials exceed the Windows vault limit".into());
        }
        let mut target = wide(TARGET);
        let mut user = wide(&value.email);
        let credential = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: target.as_mut_ptr(),
            CredentialBlobSize: bytes.len() as u32,
            CredentialBlob: bytes.as_mut_ptr(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            UserName: user.as_mut_ptr(),
            ..Default::default()
        };
        let ok = unsafe { CredWriteW(&credential, 0) } != 0;
        bytes.fill(0);
        if ok {
            Ok(())
        } else {
            Err("Windows Credential Manager could not save the Gmail connection".into())
        }
    }
    pub fn remove() -> Result<()> {
        if unsafe { CredDeleteW(wide(TARGET).as_ptr(), CRED_TYPE_GENERIC, 0) } != 0
            || unsafe { GetLastError() } == ERROR_NOT_FOUND
        {
            Ok(())
        } else {
            Err("Windows Credential Manager could not remove the Gmail connection".into())
        }
    }
}
#[cfg(not(windows))]
mod platform {
    use super::*;
    pub fn read() -> Result<Option<Credentials>> {
        Ok(None)
    }
    pub fn write(_: &Credentials) -> Result<()> {
        Err("Secure Gmail connection currently requires Windows Credential Manager".into())
    }
    pub fn remove() -> Result<()> {
        Ok(())
    }
}
pub(super) use platform::{read, remove, write};
