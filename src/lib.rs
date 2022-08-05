use anyhow::Result;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub const RAW_EXTENSIONS: [&str; 37] = [
    "nef", "3fr", "ari", "arw", "bay", "crw", "cr2", "cr3", "cap", "dcs", "dcr", "dng", "drf",
    "eip", "erf", "fff", "gpr", "mdc", "mef", "mos", "mrw", "nrw", "obm", "orf", "pef", "ptx",
    "pxn", "r3d", "raw", "rwl", "rw2", "rwz", "sr2", "srf", "srw", "x3f", "raf",
];
pub const LOSSY_EXTENSIONS: [&str; 9] = [
    "jpg", "jpeg", "png", "heic", "avif", "heif", "tiff", "tif", "hif",
];

#[derive(Debug, Clone)]
pub struct Ingestor<'ingest> {
    pub structure: Structure,
    pub target: PathBuf,
    pub sources: HashSet<&'ingest Path>,
    pub filter: Filter<'ingest>,
}

#[derive(Debug, Clone)]
pub struct Filter<'filter> {
    pub extensions: &'filter [&'filter str],
    pub min_size: u64,
    pub max_size: u64,
}

impl<'filter> Default for Filter<'filter> {
    fn default() -> Self {
        Filter {
            extensions: &[],
            min_size: 0,
            max_size: std::u64::MAX,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub enum Structure {
    /// Rename the files according to the given pattern.
    Rename(Rename),
    /// Retain the folder structure
    #[default]
    Retain,
}

#[derive(Debug, Clone, Default)]
pub enum Position {
    /// Add the
    #[default]
    Prefix,
    Suffix,
}
#[derive(Debug, Clone, Default)]
pub struct Rename {
    pub position: Position,
    pub sequence: i32,
    pub zeroes: u8,
}

impl<'ingest> Ingestor<'ingest> {
    /// Returns the free space available at the target folder
    pub fn free_space(&self) -> Result<u64> {
        Ok(fs2::free_space(&self.target)?)
    }

    /// Returns the total size of the files to be copied.
    pub fn total_size(&self) -> Result<u64> {
        Ok(self
            .files()?
            .iter()
            .map(|path| path.metadata().map(|m| m.len()).unwrap_or_default())
            .sum())
    }

    /// Returns the number of files that were ingested.
    pub fn ingest(&self) -> Result<u64> {
        for source in self.sources.iter() {

        }
    }

    /// Returns all the files that match the filters
    pub fn files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        for source in self.sources.iter() {
            files.extend(
                WalkDir::new(source)
                    .into_iter()
                    .flatten()
                    .filter_map(|entry| {
                        let path = entry.path();
                        let size = path.metadata().ok()?.len();
                        if entry.file_type().is_file()
                            && size > self.filter.min_size
                            && size < self.filter.max_size
                            && self
                                .filter
                                .extensions
                                .contains(&path.extension()?.to_str()?)
                        {
                            Some(path.to_path_buf())
                        } else {
                            None
                        }
                    }),
            )
        }
        Ok(files)
    }

    /// This returns all the folders in the source folders
    pub fn folders(&self) -> Result<Vec<PathBuf>> {
        let mut folders = Vec::new();
        for source in self.sources.iter() {
            folders.extend(
                WalkDir::new(source)
                    .into_iter()
                    .flatten()
                    .filter_map(|entry| {
                        let path = entry.path();
                        if entry.file_type().is_dir() {
                            Some(path.to_path_buf())
                        } else {
                            None
                        }
                    }),
            )
        }
        Ok(folders)
    }
}

pub fn get_size<P: AsRef<Path>>(target: P) -> Result<u64> {
    // let free_size = fs2::free_space(&target)?;
    let size = walkdir::WalkDir::new(target.as_ref().canonicalize()?)
        .into_iter()
        .flatten()
        .map(|entry| entry.path().metadata().map(|m| m.len()).unwrap_or(0))
        .sum::<u64>();
    Ok(size)
}

pub fn copy_flatten<I: AsRef<Path>, O: AsRef<Path>>(
    input: I,
    output: O,
    rename: Rename,
) -> Result<()> {
    let files = files(input)?;
    // for file in files

    Ok(())
}

/// This creates the structure as the files are being copied
pub fn copy_with_structure<I: AsRef<Path>, O: AsRef<Path>, R: AsRef<Path>>(
    root: R,
    input: I,
    output: O,
) -> Result<u64> {
    // The root is the directory where the input folder is located.
    let root = root.as_ref().canonicalize()?;
    let input = input.as_ref().canonicalize()?;
    let output = output.as_ref().canonicalize()?;
    let target = output.join(input.strip_prefix(root)?);
    Ok(std::fs::copy(input, target)?)
}

pub fn copy_directory_structure<I: AsRef<Path>, O: AsRef<Path>>(input: I, output: O) -> Result<()> {
    let input = input.as_ref().canonicalize()?;
    let output = output.as_ref().canonicalize()?;
    for folder in folders(&input)? {
        let target = output.join(folder.strip_prefix(&input)?);
        std::fs::create_dir_all(target)?;
    }
    Ok(())
}

pub fn copy_files_with_structure<I: AsRef<Path>, O: AsRef<Path>>(
    input: I,
    output: O,
) -> Result<()> {
    let path = input.as_ref();
    std::fs::create_dir_all(&output)?;
    let files = files(&path)?;
    copy_directory_structure(&path, &output)?;
    for file in files {
        copy_with_structure(&path, file, &output)?;
    }
    Ok(())
}

pub fn files(path: impl AsRef<Path>) -> Result<HashSet<PathBuf>> {
    Ok(walkdir::WalkDir::new(path.as_ref().canonicalize()?)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            if entry.file_type().is_file() {
                Some(entry.path().to_path_buf())
            } else {
                None
            }
        })
        .collect())
}

pub fn folders(path: impl AsRef<Path>) -> Result<HashSet<PathBuf>> {
    Ok(walkdir::WalkDir::new(path.as_ref().canonicalize()?)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            if entry.file_type().is_dir() {
                Some(entry.path().to_path_buf())
            } else {
                None
            }
        })
        .collect())
}

pub fn files_folders(path: impl AsRef<Path>) -> Result<(HashSet<PathBuf>, HashSet<PathBuf>)> {
    Ok(walkdir::WalkDir::new(path.as_ref().canonicalize()?)
        .into_iter()
        .flatten()
        .map(|entry| entry.path().to_path_buf())
        .partition(|path| path.is_dir()))
}

pub fn has_extension<P: AsRef<Path>, I: AsRef<str>, E: IntoIterator<Item = I>>(
    path: P,
    extensions: E,
) -> bool {
    let extension = path
        .as_ref()
        .extension()
        .and_then(OsStr::to_str)
        .map(str::to_lowercase);
    let extension = extension.as_deref();
    extensions
        .into_iter()
        .any(|ext| Some(ext.as_ref()) == extension)
}
