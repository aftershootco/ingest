use std::ffi::OsStr;
use std::path::Path;
pub trait IsJpeg {
    fn is_jpeg(&self) -> bool;
}

impl<T> IsJpeg for T
where
    T: AsRef<Path>,
{
    fn is_jpeg(&self) -> bool {
        self.as_ref()
            .extension()
            .map(OsStr::to_ascii_lowercase)
            .and_then(|ext| ext.into_string().ok())
            .map(|ext| matches!(ext.as_str(), "jpg" | "jpeg"))
            .unwrap_or_default()
    }
}
pub trait IsHidden {
    fn is_hidden(&self) -> bool;
}

impl<T> IsHidden for T
where
    T: AsRef<Path>,
{
    fn is_hidden(&self) -> bool {
        #[cfg(windows)]
        use std::os::windows::fs::MetadataExt;
        #[cfg(windows)]
        return std::fs::metadata(self.as_ref())
            .map(|m| m.file_attributes() & 0x02 != 0)
            .unwrap_or_default();
        self.as_ref()
            .file_name()
            .and_then(OsStr::to_str)
            .map(|s| s.starts_with('.'))
            .unwrap_or(true)
    }
}
