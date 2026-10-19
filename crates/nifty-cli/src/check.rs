//! `nf check`: report errors without running anything.

use std::path::Path;
use std::process::ExitCode;

use crate::frontend::SourceFile;
use crate::{CommandResult, EXIT_HAS_ERRORS, report};

pub(crate) fn run(target: &Path) -> CommandResult {
    let source = SourceFile::read(target)?;
    match source.compile() {
        Ok(_) => {
            println!("{}: no problems found", source.name());
            Ok(ExitCode::SUCCESS)
        }
        Err(diagnostics) => {
            report::print(&source, &diagnostics)?;
            Ok(ExitCode::from(EXIT_HAS_ERRORS))
        }
    }
}
