//! What a command operates on: a project folder with a `main.nf`, or a single `.nf` file.

use std::path::{Path, PathBuf};

use crate::error::CliError;

pub(crate) const SOURCE_EXTENSION: &str = "nf";
pub(crate) const ENTRY_FILE: &str = "main.nf";

/// The source file that a command target refers to.
pub(crate) fn entry_file(target: &Path) -> Result<PathBuf, CliError> {
    if target.is_dir() {
        let entry = target.join(ENTRY_FILE);
        return if entry.is_file() {
            Ok(entry)
        } else {
            Err(CliError::new(format!(
                "{} has no {ENTRY_FILE}; create one or run `nf create <name>`",
                target.display()
            )))
        };
    }
    if target.is_file()
        && target
            .extension()
            .is_some_and(|ext| ext == SOURCE_EXTENSION)
    {
        return Ok(target.to_path_buf());
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
    fn a_folder_resolves_to_its_main_file() {
        let dir = scratch_dir("folder");
        fs::write(dir.join(ENTRY_FILE), "").expect("write main.nf");
        assert_eq!(
            entry_file(&dir).expect("folder has main.nf"),
            dir.join(ENTRY_FILE)
        );
    }

    #[test]
    fn a_folder_without_main_is_an_error() {
        let dir = scratch_dir("empty");
        let error = entry_file(&dir).expect_err("no main.nf");
        assert!(error.to_string().contains("has no main.nf"));
    }

    #[test]
    fn only_nf_files_are_accepted() {
        let dir = scratch_dir("files");
        let source = dir.join("app.nf");
        let other = dir.join("notes.txt");
        fs::write(&source, "").expect("write app.nf");
        fs::write(&other, "").expect("write notes.txt");

        assert_eq!(entry_file(&source).expect("an .nf file"), source);
        assert!(entry_file(&other).is_err());
        assert!(entry_file(&dir.join("missing.nf")).is_err());
    }
}
