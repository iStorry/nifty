//! Reading a project and turning it into a checked program, shared by every command.

use std::fs;
use std::path::{Path, PathBuf};

use nifty_check::hir::Program;
use nifty_syntax::Diagnostic;

use crate::error::CliError;
use crate::project;

/// A project's entry file and its text.
pub(crate) struct SourceFile {
    pub path: PathBuf,
    pub text: String,
}

impl SourceFile {
    pub(crate) fn read(target: &Path) -> Result<Self, CliError> {
        let path = project::entry_file(target)?;
        let text = fs::read_to_string(&path)
            .map_err(|error| CliError::new(format!("can't read {}: {error}", path.display())))?;
        Ok(Self { path, text })
    }

    pub(crate) fn name(&self) -> String {
        self.path.display().to_string()
    }

    pub(crate) fn compile(&self) -> Result<Program, Vec<Diagnostic>> {
        nifty_check::compile(&nifty_syntax::parse(&self.text))
    }
}
