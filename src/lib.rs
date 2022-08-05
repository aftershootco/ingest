use anyhow::Result;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// This creates the structure as the files are being copied
pub fn copy_with_structure<I: AsRef<Path>, O: AsRef<Path>, R: AsRef<Path>>(
    root: R,
    input: I,
    output: O,
) -> Result<u64> {
    // The root is the directory where the input folder is located.
    let root = root.as_ref().canonicalize().unwrap();
    let input = input.as_ref().canonicalize().unwrap();
    let output = output.as_ref().canonicalize().unwrap();
    let target = output.join(input.strip_prefix(root).unwrap());
    Ok(std::fs::copy(input, target).unwrap())
}

pub fn copy_directory_structure<I: AsRef<Path>, O: AsRef<Path>>(input: I, output: O) -> Result<()> {
    let input = input.as_ref().canonicalize().unwrap();
    let output = output.as_ref().canonicalize().unwrap();
    for folder in folders(&input).unwrap() {
        let target = output.join(folder.strip_prefix(&input).unwrap());
        std::fs::create_dir_all(target).unwrap();
    }
    Ok(())
}

pub fn copy_files_with_structure<I: AsRef<Path>, O: AsRef<Path>>(
    input: I,
    output: O,
) -> Result<()> {
    let path = input.as_ref();
    let files = files(&path).unwrap();
    copy_directory_structure(&path, &output).unwrap();
    for file in files {
        copy_with_structure(&path, file, &output).unwrap();
    }
    Ok(())
}

pub fn files(path: impl AsRef<Path>) -> Result<HashSet<PathBuf>> {
    Ok(walkdir::WalkDir::new(path.as_ref().canonicalize().unwrap())
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            if entry.file_type().is_file() {
                let f = entry.path().to_path_buf();
                // println!("{:?}", f);
                Some(f)
            } else {
                None
            }
        })
        .collect())
}

pub fn folders(path: impl AsRef<Path>) -> Result<HashSet<PathBuf>> {
    Ok(walkdir::WalkDir::new(path.as_ref().canonicalize().unwrap())
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
    Ok(walkdir::WalkDir::new(path.as_ref().canonicalize().unwrap())
        .into_iter()
        .flatten()
        .map(|entry| entry.path().to_path_buf())
        .partition(|path| path.is_dir()))
}
