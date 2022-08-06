mod errors;
use errors::Error;
use errors::Result;
use std::borrow::Cow;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
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

#[derive(Debug, Clone, Default)]
pub struct IngestorBuilder<'ingest> {
    pub structure: Option<Structure>,
    pub target: Option<PathBuf>,
    pub sources: Option<HashSet<PathBuf>>,
    pub filter: Option<Filter<'ingest>>,
    pub copy_xmp: Option<bool>,
    pub copy_jpg: Option<bool>,
}

impl<'ingest> IngestorBuilder<'ingest> {
    pub fn with_structure(mut self, structure: Structure) -> Self {
        self.structure = Some(structure);
        self
    }
    pub fn with_target<P: AsRef<Path>>(mut self, target: P) -> Self {
        self.target = Some(target.as_ref().to_path_buf());
        self
    }
    pub fn with_source<
        P: IntoIterator<Item = PI>,
        PI: AsRef<Path> + std::hash::Hash + std::cmp::Eq + 'ingest,
    >(
        mut self,
        sources: P,
    ) -> Self {
        self.sources = Some(sources.into_iter().map(|p| p.as_ref().to_owned()).collect());
        self
    }

    pub fn copy_xmp(mut self, copy_xmp: bool) -> Self {
        self.copy_xmp = Some(copy_xmp);
        self
    }

    pub fn copy_jpg(mut self, copy_jpg: bool) -> Self {
        self.copy_jpg = Some(copy_jpg);
        self
    }

    pub fn build(self) -> Result<Ingestor<'ingest>> {
        if let Self {
            structure: Some(structure),
            target: Some(target),
            sources: Some(sources),
            filter: Some(filter),
            copy_xmp: Some(copy_xmp),
            copy_jpg: Some(copy_jpg),
        } = self
        {
            Ok(Ingestor {
                structure,
                target,
                sources,
                filter,
                copy_xmp,
                copy_jpg,
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
    pub structure: Structure,
    pub target: PathBuf,
    pub sources: HashSet<PathBuf>,
    pub filter: Filter<'ingest>,
    pub copy_xmp: bool,
    pub copy_jpg: bool,
}

#[derive(Debug, Clone)]
pub struct Filter<'filter> {
    pub extensions: Cow<'filter, [&'filter str]>,
    pub min_size: u64,
    pub max_size: u64,
}

impl<'filter> Filter<'filter> {
    pub fn matches(&self, path: impl AsRef<Path>) -> Result<bool> {
        let len = path.as_ref().metadata()?.len();
        let ext = path.as_ref().extension().and_then(OsStr::to_str);
        if let Some(ext) = ext {
            if self.extensions.contains(&ext) && len >= self.min_size && len <= self.max_size {
                return Ok(true);
            }
        } else if (self.extensions.is_empty() || self.extensions.contains(&""))
            && len >= self.min_size
            && len <= self.max_size
        {
            return Ok(true);
        }
        Ok(false)
    }
    fn images() -> Self {
        let extensions = [RAW_EXTENSIONS.as_slice(), LOSSY_EXTENSIONS.as_slice()].concat();
        Filter {
            extensions: Cow::Owned(extensions),
            min_size: 0,
            max_size: std::u64::MAX,
        }
    }
}

impl<'filter> Default for Filter<'filter> {
    fn default() -> Self {
        Filter {
            extensions: Default::default(),
            min_size: 0,
            max_size: std::u64::MAX,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub enum Structure {
    /// Rename the files according to the given pattern.
    Rename(Rename),
    /// Preserve the names not the folder structure
    Preserve,
    /// Retain the folder structure
    #[default]
    Retain,
}

impl Structure {
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
#[derive(Debug, Clone, Default)]
pub struct Rename {
    pub name: Option<String>,
    pub position: Position,
    pub sequence: i32,
    pub zeroes: u8,
}

impl Rename {
    pub fn file_stem(&self, path: impl AsRef<Path>) -> Result<String> {
        let name = self.name.clone().unwrap_or(
            path.as_ref()
                .file_stem()
                .and_then(OsStr::to_str)
                .map(|m| m.to_owned())
                .ok_or_else(|| Error::custom_error("File stem not found"))?,
        );
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
    pub fn ingest(&mut self) -> Result<u64> {
        fs::create_dir_all(&self.target)?;
        let mut rename = match self.structure.clone() {
            Structure::Rename(ref rename) => Some(rename.clone()),
            _ => None,
        }
        .unwrap_or_default();

        for source in self.sources.clone().iter() {
            WalkDir::new(source)
                .into_iter()
                .flatten()
                .try_for_each(|entry| -> Result<()> {
                    let path = entry.path();
                    if self.filter.matches(path)? {
                        match self.structure {
                            Structure::Retain => self.ingest_file(source, path).ok(),
                            Structure::Rename(_) => {
                                self.ingest_file_renamed(path, &mut rename).ok()
                            }
                            // Structure::Preserve => self.ingest_file_preserve(path).ok(),
                            Structure::Preserve => todo!(),
                        };
                    }
                    Ok(())
                })?;
        }
        Ok(0)
    }

    fn ingest_file<P: AsRef<Path>, S: AsRef<Path>>(&self, source: S, path: P) -> Result<()> {
        let source = source.as_ref();
        // if the source folder is
        // aaa/bbb
        // and the file is
        // aaa/bbb/ccc/ddd.jpg
        // then the target is
        // xxx/yyy
        // then the target file must be
        // xxx/yyy/bbb/ccc/ddd.jpg
        //
        // source
        // /
        // file
        // /aaa/bbb/file.jpg
        // target
        // xxx/yyy
        // target file
        // xxx/yyy/aaa/bbb/file.jpg
        let target = if let Some(source) = source.parent() {
            self.target.join(path.as_ref().strip_prefix(source)?)
        } else {
            self.target.join(path.as_ref().strip_prefix(source)?)
        };
        fs::create_dir_all(target.parent().unwrap())?;
        self.ingest_copy(&path, &target)?;

        Ok(())
    }

    pub fn ingest_file_renamed<P: AsRef<Path>>(
        &mut self,
        path: P,
        rename: &mut Rename,
    ) -> Result<()> {
        let file_extension = path
            .as_ref()
            .extension()
            .and_then(OsStr::to_str)
            .ok_or_else(|| Error::custom_error("File extension not found"))?;

        let target =
            self.target
                .canonicalize()?
                .join(format!("{}.{}", rename.next(&path)?, file_extension));
        self.ingest_copy(path, target)?;
        Ok(())
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
                        if entry.file_type().is_file() && self.filter.matches(path).ok()? {
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
        // let mut folders = Vec::new();
        // for source in self.sources.iter() {
        // folders.extend(
        let folders = self
            .sources
            .clone()
            .into_iter()
            .fold(Vec::new(), |mut last, source| {
                last.extend(
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
                );
                last
            });
        // .collect();
        // )
        // }
        Ok(folders)
    }

    pub fn builder() -> IngestorBuilder<'ingest> {
        Default::default()
    }
    pub fn ingest_copy<I: AsRef<Path>, O: AsRef<Path>>(&self, input: I, output: O) -> Result<u64> {
        if self.copy_xmp {
            fs::copy(
                input.as_ref().with_extension("xmp"),
                output.as_ref().with_extension("xmp"),
            )
            .ok();
        }
        if self.copy_jpg {
            fs::copy(
                input.as_ref().with_extension("jpg"),
                output.as_ref().with_extension("jpg"),
            )
            .ok();
        }
        Ok(fs::copy(input, output)?)
    }
}
