//! `nf check`: report errors without running anything.

use std::path::Path;
use std::process::ExitCode;

use crate::frontend::Sources;
use crate::{CommandResult, EXIT_HAS_ERRORS, report};

pub(crate) fn run(target: &Path) -> CommandResult {
    let sources = Sources::read(target)?;
    match sources.compile() {
        Ok(_) => {
            println!("{}: no problems found", sources.name());
            Ok(ExitCode::SUCCESS)
        }
        Err(diagnostics) => {
            report::print(&sources, &diagnostics)?;
            Ok(ExitCode::from(EXIT_HAS_ERRORS))
        }
    }
}
