use std::{io, path::Path};

#[cfg(not(windows))]
pub(super) fn move_file(source: &Path, destination: &Path, _overwrite: bool) -> io::Result<()> {
    std::fs::rename(source, destination)
}

#[cfg(windows)]
pub(super) fn move_file(source: &Path, destination: &Path, overwrite: bool) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_COPY_ALLOWED, MOVEFILE_REPLACE_EXISTING, MoveFileExW,
    };

    let source = source
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let flags = MOVEFILE_COPY_ALLOWED
        | if overwrite {
            MOVEFILE_REPLACE_EXISTING
        } else {
            0
        };

    // SAFETY: Both path buffers are NUL-terminated and remain alive for the duration of the call.
    let result = unsafe { MoveFileExW(source.as_ptr(), destination.as_ptr(), flags) };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
