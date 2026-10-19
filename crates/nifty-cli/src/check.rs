//! `nf check`: report errors without running anything.

use std::fs;
use std::path::Path;
use std::process::ExitCode;

use crate::error::CliError;
use crate::{CommandResult, project, report};

const EXIT_HAS_ERRORS: u8 = 1;

pub(crate) fn run(target: &Path) -> CommandResult {
    let entry = project::entry_file(target)?;
    let source = fs::read_to_string(&entry)
        .map_err(|error| CliError::new(format!("can't read {}: {error}", entry.display())))?;

    let parsed = nifty_syntax::parse(&source);
    if parsed.diagnostics.is_empty() {
        println!("{}: no problems found", entry.display());
        return Ok(ExitCode::SUCCESS);
    }

    report::print(&entry.display().to_string(), &source, &parsed.diagnostics)
        .map_err(|error| CliError::new(format!("can't write diagnostics: {error}")))?;
    Ok(ExitCode::from(EXIT_HAS_ERRORS))
}
