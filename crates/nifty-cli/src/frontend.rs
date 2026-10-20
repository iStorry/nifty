//! Reading a project and turning it into a checked program, shared by every command.

use std::fs;
use std::path::{Path, PathBuf};

use nifty_check::hir::Program;
use nifty_syntax::{Diagnostic, FileId};

use crate::error::CliError;
use crate::project::{self, ProjectFile};

/// One source file of a project and its text.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct SourceFile {
    pub path: PathBuf,
    /// Where the file sits in its project; see [`ProjectFile::relative`].
    pub relative: PathBuf,
    pub text: String,
}

impl SourceFile {
    pub(crate) fn name(&self) -> String {
        self.path.display().to_string()
    }
}

/// Every source file of a command target, the entry file first. File `i` is
/// `FileId::new(i)` in the program's spans.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Sources {
    target: PathBuf,
    files: Vec<SourceFile>,
}

impl Sources {
    pub(crate) fn read(target: &Path) -> Result<Self, CliError> {
        let files = project::source_files(target)?
            .into_iter()
            .map(|ProjectFile { path, relative }| {
                let text = fs::read_to_string(&path).map_err(|error| {
                    CliError::new(format!("can't read {}: {error}", path.display()))
                })?;
                Ok(SourceFile {
                    path,
                    relative,
                    text,
                })
            })
            .collect::<Result<_, CliError>>()?;
        Ok(Self {
            target: target.to_path_buf(),
            files,
        })
    }

    /// The project folder or single file, as the user named it.
    pub(crate) fn name(&self) -> String {
        self.target.display().to_string()
    }

    pub(crate) fn entry(&self) -> &SourceFile {
        self.file(FileId::default())
    }

    pub(crate) fn files(&self) -> &[SourceFile] {
        &self.files
    }

    /// The file a span points into.
    pub(crate) fn file(&self, id: FileId) -> &SourceFile {
        &self.files[id.index()]
    }

    pub(crate) fn compile(&self) -> Result<Program, Vec<Diagnostic>> {
        let parsed: Vec<_> = self
            .files
            .iter()
            .enumerate()
            .map(|(index, file)| nifty_syntax::parse(&file.text, FileId::new(index)))
            .collect();
        nifty_check::compile(&parsed.iter().collect::<Vec<_>>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPLIT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/split");

    #[test]
    fn a_project_split_across_files_compiles_as_one_program() {
        let sources = Sources::read(Path::new(SPLIT)).expect("the fixture is readable");
        let relative: Vec<&Path> = sources
            .files()
            .iter()
            .map(|file| file.relative.as_path())
            .collect();
        assert_eq!(relative, [Path::new("main.nf"), Path::new("ui/header.nf")]);
        let program = sources.compile().expect("the fixture checks");
        assert_eq!(program.views[0].name, "Header");
    }
}
