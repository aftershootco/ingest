mod errors;
mod traits;
use std::sync::{atomic::AtomicUsize, Arc};

mod ingest;
pub use ingest::*;

pub use errors::Error;
use errors::Result;
use std::borrow::Cow;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
pub(crate) use traits::IsHidden;
use traits::IsJpeg;
use walkdir::WalkDir;

pub const RAW_EXTENSIONS: [&str; 37] = [
    "nef", "3fr", "ari", "arw", "bay", "crw", "cr2", "cr3", "cap", "dcs", "dcr", "dng", "drf",
    "eip", "erf", "fff", "gpr", "mdc", "mef", "mos", "mrw", "nrw", "obm", "orf", "pef", "ptx",
    "pxn", "r3d", "raw", "rwl", "rw2", "rwz", "sr2", "srf", "srw", "x3f", "raf",
];
pub const LOSSY_EXTENSIONS: [&str; 9] = [
    "jpg", "jpeg", "png", "heic", "avif", "heif", "tiff", "tif", "hif",
];

#[derive(Debug, Clone, Default)]
pub struct IngestorBuilder<'ingest> {
    pub structure: Option<Structure<'ingest>>,
    pub target: Option<PathBuf>,
    pub backup: Option<PathBuf>,
    pub sources: Option<HashSet<&'ingest Path>>,
    pub filter: Option<Filter<'ingest>>,
    pub copy_xmp: Option<bool>,
    pub copy_jpg: Option<bool>,
    pub ignore_hidden: Option<bool>,
    pub progress: Option<Arc<AtomicUsize>>,
    pub depth: Option<usize>,
}

impl<'ingest> IngestorBuilder<'ingest> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_structure(&mut self, structure: Structure<'ingest>) -> &mut Self {
        self.structure = Some(structure);
        self
    }

    pub fn with_depth(&mut self, depth: usize) -> &mut Self {
        self.depth = Some(depth);
        self
    }

    pub fn with_target<P: AsRef<Path>>(&mut self, target: P) -> &mut Self {
        self.target = Some(target.as_ref().to_path_buf());
        self
    }
    pub fn with_source<
        P: IntoIterator<Item = &'ingest PI>,
        PI: AsRef<Path> + std::hash::Hash + std::cmp::Eq + 'ingest,
    >(
        &mut self,
        sources: P,
    ) -> &mut Self {
        self.sources = Some(sources.into_iter().map(|p| p.as_ref()).collect());
        self
    }

    pub fn with_filter(&mut self, filter: impl Into<Filter<'ingest>>) -> &mut Self {
        self.filter = Some(filter.into());
        self
    }

    pub fn progress(&mut self, progress: Arc<AtomicUsize>) -> &mut Self {
        self.progress = Some(progress);
        self
    }

    pub fn copy_xmp(&mut self, copy_xmp: bool) -> &mut Self {
        self.copy_xmp = Some(copy_xmp);
        self
    }

    pub fn copy_jpg(&mut self, copy_jpg: bool) -> &mut Self {
        self.copy_jpg = Some(copy_jpg);
        self
    }

    pub fn backup<P: AsRef<Path>>(&mut self, backup: P) -> &mut Self {
        self.backup = Some(backup.as_ref().to_path_buf());
        self
    }

    pub fn build(&self) -> Result<Ingestor<'ingest>> {
        let ingestor = self.to_owned();
        if let Self {
            structure: Some(structure),
            target: Some(target),
            sources: Some(sources),
            filter: Some(filter),
            backup,
            ..
        } = ingestor
        {
            Ok(Ingestor {
                structure,
                target,
                sources,
                filter,
                backup,
                copy_xmp: ingestor.copy_xmp.unwrap_or(true),
                copy_jpg: ingestor.copy_jpg.unwrap_or(true),
                progress: ingestor.progress.unwrap_or_default(),
                depth: ingestor.depth.unwrap_or(usize::MAX),
                ..Default::default()
            })
        } else {
            Err(Error::custom_error("Missing required fields"))
        }
    }

    pub fn images() -> Self {
        Self {
            filter: Some(Filter::images()),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Ingestor<'ingest> {
    pub structure: Structure<'ingest>,
    pub target: PathBuf,
    pub backup: Option<PathBuf>,
    pub sources: HashSet<&'ingest Path>,
    pub filter: Filter<'ingest>,
    pub copy_xmp: bool,
    pub copy_jpg: bool,
    pub progress: Arc<AtomicUsize>,
    pub depth: usize,
    __jpegs: HashSet<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct Filter<'filter> {
    pub extensions: Cow<'filter, [&'filter str]>,
    pub min_size: u64,
    pub max_size: u64,
    pub ignore_hidden: bool,
}

impl<'filter> Filter<'filter> {
    pub fn images() -> Self {
        let extensions = [RAW_EXTENSIONS.as_slice(), LOSSY_EXTENSIONS.as_slice()].concat();
        Filter {
            extensions: Cow::Owned(extensions),
            min_size: 0,
            max_size: std::u64::MAX,
            ignore_hidden: true,
        }
    }
    pub fn raws() -> Self {
        Filter {
            extensions: Cow::Borrowed(RAW_EXTENSIONS.as_ref()),
            min_size: 0,
            max_size: std::u64::MAX,
            ignore_hidden: true,
        }
    }

    pub fn jpegs() -> Self {
        Filter {
            extensions: Cow::Borrowed(LOSSY_EXTENSIONS.as_ref()),
            min_size: 0,
            max_size: std::u64::MAX,
            ignore_hidden: true,
        }
    }
}

impl<'filter> Default for Filter<'filter> {
    fn default() -> Self {
        Filter {
            extensions: Default::default(),
            min_size: 0,
            max_size: std::u64::MAX,
            ignore_hidden: true,
        }
    }
}

#[derive(Debug, Clone, Default, Copy)]
pub enum Structure<'structure> {
    /// Rename the files according to the given pattern.
    Rename(Rename<'structure>),
    /// Preserve the names not the folder structure
    Preserve,
    /// Retain the folder structure
    #[default]
    Retain,
}

impl<'st> Structure<'st> {
    pub fn is_retained(&self) -> bool {
        matches!(self, Structure::Retain)
    }
    pub fn is_renamed(&self) -> bool {
        matches!(self, Structure::Rename(_))
    }
    pub fn is_preserved(&self) -> bool {
        matches!(self, Structure::Preserve)
    }
}

#[derive(Debug, Clone, Default, Copy)]
pub enum Position {
    /// Add the
    #[default]
    Prefix,
    Suffix,
}
#[derive(Debug, Clone, Default, Copy)]
pub struct Rename<'ren> {
    pub name: Option<&'ren str>,
    pub position: Position,
    pub sequence: i32,
    pub zeroes: u8,
}

impl<'ren> Rename<'ren> {
    pub fn file_stem(&self, path: impl AsRef<Path>) -> Result<String> {
        // let name = self.name.clone().unwrap_or(
        //     &path
        //         .as_ref()
        //         .file_stem()
        //         .and_then(OsStr::to_str)
        //         .map(|m| m.to_owned())
        //         .ok_or_else(|| Error::custom_error("File stem not found"))?,
        // );
        let name = if let Some(name) = self.name {
            name
        } else {
            path.as_ref()
                .file_stem()
                .and_then(OsStr::to_str)
                .ok_or_else(|| Error::custom_error("File stem not found"))?
        };
        Ok(match self.position {
            Position::Suffix => format!("{}-{:0z$}", name, self.sequence, z = self.zeroes as usize),
            Position::Prefix => format!("{:0z$}-{}", self.sequence, name, z = self.zeroes as usize),
        })
    }
    pub fn next(&mut self, path: impl AsRef<Path>) -> Result<String> {
        let file_stem = self.file_stem(path);
        if file_stem.is_ok() {
            self.sequence += 1;
        }
        file_stem
    }
}

pub(crate) fn accompanying_jpeg(path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = path.as_ref();
    let extension = path
        .extension()
        .map(OsStr::to_ascii_lowercase)
        .and_then(|ext| ext.into_string().ok())
        .ok_or_else(|| Error::custom_error("File extension not found"))?;

    if matches!(extension.as_str(), "jpg" | "jpeg") {
        Err(Error::custom_error(
            "Jpeg file can't have accompanying jpeg",
        ))
    } else {
        return ["jpg", "jpeg"]
            .iter()
            .find_map(|e| path.with_extension(e).canonicalize().ok())
            .ok_or_else(|| Error::custom_error("No accompanying jpeg found"));
    }
}

pub(crate) fn exists_plus_one(path: impl AsRef<Path>) -> Result<PathBuf> {
    let original_path = path.as_ref().to_owned();
    let mut count = 1;
    let mut path = original_path.clone();
    while path.exists() {
        path = original_path.with_file_name(format!(
            "{}-{count}.{}",
            original_path
                .file_stem()
                .ok_or_else(|| Error::custom_error("File name not found"))?
                .to_string_lossy(),
            original_path
                .extension()
                .ok_or_else(|| Error::custom_error("Extension not found"))?
                .to_string_lossy()
        ));
        count += 1;
    }
    Ok(path)
}

#[cfg(unix)]
pub(crate) fn same_disk<P1: AsRef<Path>, P2: AsRef<Path>>(p1: P1, p2: P2) -> std::io::Result<bool> {
    use std::os::unix::fs::MetadataExt;
    Ok(p1.as_ref().metadata()?.dev() == p2.as_ref().metadata()?.dev())
}
#[cfg(windows)]
pub(crate) fn same_disk<P1: AsRef<Path>, P2: AsRef<Path>>(p1: P1, p2: P2) -> std::io::Result<bool> {
    Ok(p1.as_ref().canonicalize()?.components().next()
        == p2.as_ref().canonicalize()?.components().next())
}

pub struct Needs {
    pub total: u64,
    pub free: u64,
    pub backup: Option<BackupNeeds>,
}

pub struct BackupNeeds {
    pub free: u64,
    pub same_disk: bool,
}
