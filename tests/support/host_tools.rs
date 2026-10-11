//! The host's archive tools, for tests that read or probe `zt dp` archives.

use std::path::PathBuf;

/// The tar that reads every format `zt dp` writes. On Windows that is the bundled bsdtar
/// in System32: a GNU tar earlier on PATH (Git for Windows ships one) cannot read ZIP or
/// 7z archives, and it takes `C:\...` arguments for remote host names.
pub fn host_tar() -> PathBuf {
    #[cfg(windows)]
    {
        if let Some(root) = std::env::var_os("SystemRoot") {
            let tar = PathBuf::from(root).join("System32").join("tar.exe");
            if tar.is_file() {
                return tar;
            }
        }
        PathBuf::from("tar.exe")
    }
    #[cfg(not(windows))]
    {
        PathBuf::from("tar")
    }
}
