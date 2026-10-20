//! What a command operates on: a project folder with a `main.nf`, or a single `.nf` file.

use std::path::{Path, PathBuf};

use nifty_syntax::project::{self, ENTRY_FILE, SOURCE_EXTENSION};

use crate::error::CliError;

/// One source file of a command target.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ProjectFile {
    /// Where the file is, for reading it and naming it in messages.
    pub path: PathBuf,
    /// Where the file sits in its project. A single file is a project of one, so it is the
    /// project's entry file.
    pub relative: PathBuf,
}

/// The source files that a command target refers to: every `.nf` file of a project
/// folder, `main.nf` first, or a single `.nf` file on its own.
pub(crate) fn source_files(target: &Path) -> Result<Vec<ProjectFile>, CliError> {
    if target.is_dir() {
        if !target.join(ENTRY_FILE).is_file() {
            return Err(CliError::new(format!(
                "{} has no {ENTRY_FILE}; create one or run `nf create <name>`",
                target.display()
            )));
        }
        let files = project::source_files(target)
            .map_err(|error| CliError::new(format!("can't list {}: {error}", target.display())))?;
        return Ok(files
            .into_iter()
            .map(|relative| ProjectFile {
                path: target.join(&relative),
                relative,
            })
            .collect());
    }
    if target.is_file() && project::is_source(target) {
        return Ok(vec![ProjectFile {
            path: target.to_path_buf(),
            relative: PathBuf::from(ENTRY_FILE),
        }]);
    }
    Err(CliError::new(format!(
        "{} is not a Nifty project folder or .{SOURCE_EXTENSION} file",
        target.display()
    )))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::test_support::scratch_dir;

    #[test]
    fn a_folder_resolves_to_all_its_source_files_main_first() {
        let dir = scratch_dir("folder");
        fs::create_dir(dir.join("ui")).expect("create ui");
        for file in [ENTRY_FILE, "ui/header.nf", "app.nf"] {
            fs::write(dir.join(file), "").expect("write a source file");
        }
        let files = source_files(&dir).expect("folder has main.nf");
        assert_eq!(files[0].path, dir.join(ENTRY_FILE));
        let relative: Vec<&Path> = files.iter().map(|file| file.relative.as_path()).collect();
        assert_eq!(
            relative,
            [ENTRY_FILE, "app.nf", "ui/header.nf"].map(Path::new)
        );
    }

    #[test]
    fn a_folder_without_main_is_an_error() {
        let dir = scratch_dir("empty");
        fs::write(dir.join("app.nf"), "").expect("write app.nf");
        let error = source_files(&dir).expect_err("no main.nf");
        assert!(error.to_string().contains("has no main.nf"));
    }

    #[test]
    fn a_single_file_stays_on_its_own() {
        let dir = scratch_dir("files");
        let source = dir.join("app.nf");
        let other = dir.join("notes.txt");
        fs::write(dir.join(ENTRY_FILE), "").expect("write main.nf");
        fs::write(&source, "").expect("write app.nf");
        fs::write(&other, "").expect("write notes.txt");

        let files = source_files(&source).expect("an .nf file");
        assert_eq!(
            files,
            [ProjectFile {
                path: source,
                relative: PathBuf::from(ENTRY_FILE)
            }]
        );
        assert!(source_files(&other).is_err());
        assert!(source_files(&dir.join("missing.nf")).is_err());
    }
}
