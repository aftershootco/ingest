use crate::*;
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
        if self.free_space()? < self.total_size()? {
            return Err(Error::custom_error("Not enough space"));
        }
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
                                if path.is_jpeg() {
                                    let path = path.to_path_buf();
                                    if self.__jpegs.contains(&path) {
                                        self.__jpegs.remove(&path);
                                        return Ok(());
                                    } else {
                                        self.__jpegs.insert(path);
                                    }
                                };
                                self.ingest_file_renamed(path, &mut rename).ok()
                            }
                            // Structure::Preserve => self.ingest_file_preserve(path).ok(),
                            Structure::Preserve => todo!(),
                        };
                    }
                    Ok(())
                })?;
        }

        let jpegs: Vec<PathBuf> = self.__jpegs.drain().collect();
        let __copy_xmp = self.copy_xmp;
        let __copy_jpg = self.copy_jpg;
        for jpeg in jpegs {
            self.copy_xmp = false;
            self.copy_jpg = false;
            match self.structure {
                Structure::Retain => {
                    self.ingest_file_renamed(jpeg, &mut rename).ok();
                }
                _ => (),
            };
        }

        if let Some(backup) = &self.backup {
            self.copy_xmp = __copy_xmp;
            self.copy_jpg = __copy_jpg;
            self.target = backup.to_owned();
            self.backup = None;
            self.ingest()?;
        }

        Ok(0)
    }

    /// This copies the files as is
    fn ingest_file<P: AsRef<Path>, S: AsRef<Path>>(&mut self, source: S, path: P) -> Result<()> {
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

    /// Since this doesn't retain the structure we need to rename the accompanying jpegs as well
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

    pub fn ingest_file_preserve<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let target = self.target.canonicalize()?.join(
            path.as_ref()
                .file_name()
                .ok_or_else(|| Error::custom_error("File name not found"))?,
        );
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

    pub fn ingest_copy<I: AsRef<Path>, O: AsRef<Path>>(
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
            .ok();
        }
        if !self.structure.is_retained() && self.copy_jpg {
            if let Ok(path) = accompanying_jpeg(&input) {
                if self.__jpegs.contains(&path) {
                    self.__jpegs.remove(&path);
                } else {
                    self.__jpegs.insert(path.clone());
                }
                fs::copy(path, output.with_extension("jpg")).ok();
            }
        }

        Ok(fs::copy(input, output)?)
    }
}
