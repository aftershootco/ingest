use crate::*;
use tokio::fs;

impl<'filter> Filter<'filter> {
    pub fn matches(&self, path: impl AsRef<Path>) -> Result<bool> {
        if path.is_hidden() == self.ignore_hidden {
            return Ok(false);
        }

        let ext = path
            .as_ref()
            .extension()
            .map(OsStr::to_ascii_lowercase)
            .and_then(|ext| ext.into_string().ok());
        let ext = ext.as_deref();

        let size = path.as_ref().metadata()?.len();
        if let Some(ext) = ext {
            if self.extensions.contains(&ext) && size >= self.min_size && size <= self.max_size {
                return Ok(true);
            }
        } else if (self.extensions.is_empty() || self.extensions.contains(&""))
            && size >= self.min_size
            && size <= self.max_size
        {
            return Ok(true);
        }
        Ok(false)
    }
}

impl<'ingest> Ingestor<'ingest> {
    /// Returns the free space available at the target folder
    pub fn free_space(&self) -> Result<u64> {
        std::fs::create_dir_all(&self.target)?;
        Ok(fs2::free_space(&self.target)?)
    }

    /// Returns the total space available at the target folder
    pub fn free_space_backup(&self) -> Result<u64> {
        if let Some(ref backup) = self.backup {
            std::fs::create_dir_all(backup)?;
            Ok(fs2::free_space(backup)?)
        } else {
            Err(Error::custom_error("Backup directory not set"))
        }
    }

    /// Returns the total size of the files to be copied.
    pub fn total_size(&self) -> Result<u64> {
        Ok(self
            .files()?
            .iter()
            .map(|path| path.metadata().map(|m| m.len()).unwrap_or_default())
            .sum())
    }

    pub async fn fits(&self) -> Result<bool> {
        Ok(if let Some(ref backup_dir) = self.backup {
            fs::create_dir_all(backup_dir).await?;
            if same_disk(&self.target, backup_dir)? {
                self.free_space()? < self.total_size()? * 2
            } else {
                let total_size = self.total_size()?;
                self.free_space()? < total_size || self.free_space_backup()? < total_size
            }
        } else {
            self.free_space()? < self.total_size()?
        })
    }

    /// Returns the number of files that were ingested.
    pub async fn ingest(&mut self) -> Result<()> {
        if !self.fits().await? {
            return Err(Error::new(errors::ErrorKind::InsufficientSpace));
        }

        let mut rename = match self.structure {
            Structure::Rename(ref rename) => Some(*rename),
            _ => None,
        }
        .unwrap_or_default();

        // TODO: futures::future::try_join_all
        for source in self.sources.clone().iter() {
            for entry in WalkDir::new(source).into_iter().flatten() {
                self.map_entry(entry, &source, &mut rename).await?;
            }
        }

        let jpegs: Vec<PathBuf> = self.__jpegs.drain().collect();
        let __copy_xmp = self.copy_xmp;
        let __copy_jpg = self.copy_jpg;
        for jpeg in jpegs {
            self.copy_xmp = false;
            self.copy_jpg = false;
            match self.structure {
                Structure::Retain => {
                    self.ingest_file_renamed(jpeg, &mut rename).await.ok();
                }
                _ => (),
            };
        }
        self.copy_xmp = __copy_xmp;
        self.copy_jpg = __copy_jpg;
        self.backup().await?;

        Ok(())
    }

    /// Returns the number of files that were ingested.
    pub async fn backup(&mut self) -> Result<()> {
        if let Some(backup) = &self.backup {
            self.target = backup.to_owned();
            self.backup = None;
        } else {
            return Ok(());
        }
        fs::create_dir_all(&self.target).await?;
        if self.free_space()? < self.total_size()? {
            return Err(Error::new(errors::ErrorKind::InsufficientSpace));
        }
        let mut rename = match self.structure {
            Structure::Rename(ref rename) => Some(*rename),
            _ => None,
        }
        .unwrap_or_default();

        // TODO: futures::future::try_join_all
        for source in self.sources.clone().iter() {
            for entry in WalkDir::new(source).into_iter().flatten() {
                self.map_entry(entry, &source, &mut rename).await?;
            }
        }

        let jpegs: Vec<PathBuf> = self.__jpegs.drain().collect();
        let __copy_xmp = self.copy_xmp;
        let __copy_jpg = self.copy_jpg;
        for jpeg in jpegs {
            self.copy_xmp = false;
            self.copy_jpg = false;
            match self.structure {
                Structure::Retain => {
                    self.ingest_file_renamed(jpeg, &mut rename).await.ok();
                }
                _ => (),
            };
        }

        Ok(())
    }

    /// This copies the files as is
    async fn ingest_file<P: AsRef<Path>, S: AsRef<Path>>(
        &mut self,
        source: S,
        path: P,
    ) -> Result<()> {
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
        fs::create_dir_all(target.parent().unwrap()).await?;
        self.ingest_copy(&path, &target).await?;

        Ok(())
    }

    /// Since this doesn't retain the structure we need to rename the accompanying jpegs as well
    pub async fn ingest_file_renamed<P: AsRef<Path>>(
        &mut self,
        path: P,
        rename: &mut Rename<'ingest>,
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
        self.ingest_copy(path, target).await?;
        Ok(())
    }

    pub async fn ingest_file_preserve<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let target = self.target.canonicalize()?.join(
            path.as_ref()
                .file_name()
                .ok_or_else(|| Error::custom_error("File name not found"))?,
        );
        self.ingest_copy(path, target).await?;
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

    pub async fn ingest_copy<I: AsRef<Path>, O: AsRef<Path>>(
        &mut self,
        input: I,
        output: O,
    ) -> Result<u64> {
        let output = crate::exists_plus_one(output)?;

        if self.copy_xmp {
            fs::copy(
                input.as_ref().with_extension("xmp"),
                output.with_extension("xmp"),
            )
            .await
            .ok();
        }
        if !self.structure.is_retained() && self.copy_jpg {
            if let Ok(path) = accompanying_jpeg(&input) {
                if self.__jpegs.contains(&path) {
                    self.__jpegs.remove(&path);
                } else {
                    self.__jpegs.insert(path.clone());
                }
                fs::copy(path, output.with_extension("jpg")).await.ok();
            }
        }

        Ok(fs::copy(input, output).await?)
    }

    pub async fn map_entry(
        &mut self,
        entry: walkdir::DirEntry,
        source: impl AsRef<Path>,
        rename: &mut Rename<'ingest>,
    ) -> Result<()> {
        self.progress
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let path = entry.path();
        if self.filter.matches(path)? {
            match self.structure {
                Structure::Retain => self.ingest_file(source, path).await.ok(),
                Structure::Rename(_) => {
                    if path.is_jpeg() {
                        let path = path.to_path_buf();
                        if self.__jpegs.contains(&path) {
                            self.__jpegs.remove(&path);
                            return Ok(());
                        } else {
                            self.__jpegs.insert(path);
                        }
                    };
                    self.ingest_file_renamed(path, rename).await.ok()
                }
                // Structure::Preserve => self.ingest_file_preserve(path).ok(),
                Structure::Preserve => todo!(),
            };
        }
        Ok(())
    }
}
