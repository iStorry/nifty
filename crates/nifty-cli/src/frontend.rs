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

    /// Parses and checks the file. Type checking only runs on a file that parses, so one
    /// syntax error doesn't cause a cascade of confusing type errors.
    pub(crate) fn compile(&self) -> Result<Program, Vec<Diagnostic>> {
        let parsed = nifty_syntax::parse(&self.text);
        if !parsed.diagnostics.is_empty() {
            return Err(parsed.diagnostics);
        }
        let checked = nifty_check::check(&parsed.file);
        if checked.diagnostics.is_empty() {
            Ok(checked.program)
        } else {
            Err(checked.diagnostics)
        }
    }
}
