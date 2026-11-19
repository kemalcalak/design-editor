//! Whether this process runs from an MSIX package (the Microsoft Store
//! build). Such an install is updated by the Store: its files are
//! read-only, and the Store owns the update, so the in-app update check
//! stays off.

/// `true` when Windows reports a package identity for this process.
#[cfg(target_os = "windows")]
pub fn is_msix_packaged() -> bool {
    use windows_sys::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER;
    use windows_sys::Win32::Storage::Packaging::Appx::GetCurrentPackageFullName;

    let mut len: u32 = 0;
    // SAFETY: a zero length with a null buffer is the documented size
    // query; Windows writes only `len`.
    let status = unsafe { GetCurrentPackageFullName(&mut len, std::ptr::null_mut()) };
    // An unpackaged process gets APPMODEL_ERROR_NO_PACKAGE; a packaged one
    // gets ERROR_INSUFFICIENT_BUFFER for the size query.
    status == ERROR_INSUFFICIENT_BUFFER
}

/// Only Windows has MSIX packages.
#[cfg(not(target_os = "windows"))]
pub fn is_msix_packaged() -> bool {
    false
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_test_binary_is_not_packaged() {
        assert!(!super::is_msix_packaged());
    }
}
