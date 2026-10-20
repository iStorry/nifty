//! Which files make up a project: the folder holding `main.nf` and every `.nf` file below it.

use std::cmp::Reverse;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Marks a folder as a project, and is the first of its files.
pub const ENTRY_FILE: &str = "main.nf";
pub const SOURCE_EXTENSION: &str = "nf";

#[must_use]
pub fn is_source(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == SOURCE_EXTENSION)
}

/// The project `file` belongs to: the nearest folder above it that holds a `main.nf`.
#[must_use]
pub fn root_of(file: &Path) -> Option<&Path> {
    file.ancestors()
        .skip(1)
        .find(|folder| folder.join(ENTRY_FILE).is_file())
}

/// The source files of the project in `root`, relative to it: `main.nf` first, then every
/// other `.nf` file in it and its subfolders, sorted. A subfolder with its own `main.nf` is
/// a separate project, such as an app `nf build` made, so its files are left out.
///
/// # Errors
///
/// Fails when a folder in the project can't be read.
pub fn source_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect(root, Path::new(""), &mut files)?;
    files.sort_by_key(|path| (Reverse(path == Path::new(ENTRY_FILE)), path.clone()));
    Ok(files)
}

fn collect(root: &Path, folder: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(root.join(folder))? {
        let entry = entry?;
        let path = folder.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            if !root.join(&path).join(ENTRY_FILE).is_file() {
                collect(root, &path, files)?;
            }
        } else if is_source(&path) {
            files.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, relative: &str) {
        let path = root.join(relative);
        if let Some(folder) = path.parent() {
            fs::create_dir_all(folder).expect("create folder");
        }
        fs::write(path, "").expect("write file");
    }

    #[test]
    fn lists_main_first_then_every_other_source_file_sorted() {
        let root = tempfile::tempdir().expect("scratch folder");
        for file in [
            "main.nf",
            "b.nf",
            "a.nf",
            "ui/z.nf",
            "ui/deep/y.nf",
            "notes.txt",
        ] {
            write(root.path(), file);
        }
        let files = source_files(root.path()).expect("the project is readable");
        let expected = ["main.nf", "a.nf", "b.nf", "ui/deep/y.nf", "ui/z.nf"];
        assert_eq!(files, expected.map(PathBuf::from));
    }

    #[test]
    fn leaves_out_folders_that_are_projects_of_their_own() {
        let root = tempfile::tempdir().expect("scratch folder");
        for file in ["main.nf", "build/app.app/main.nf", "build/app.app/ui.nf"] {
            write(root.path(), file);
        }
        let files = source_files(root.path()).expect("the project is readable");
        assert_eq!(files, [PathBuf::from(ENTRY_FILE)]);
    }

    #[test]
    fn a_file_belongs_to_the_nearest_folder_with_a_main_file() {
        let root = tempfile::tempdir().expect("scratch folder");
        write(root.path(), "main.nf");
        write(root.path(), "ui/header.nf");
        let nested = root.path().join("ui/header.nf");
        assert_eq!(root_of(&nested), Some(root.path()));
        assert_eq!(root_of(&root.path().join(ENTRY_FILE)), Some(root.path()));
    }
}
