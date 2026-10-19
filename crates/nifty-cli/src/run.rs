//! `nf run`: check the app, then run it with native windows.

use std::path::Path;
use std::process::{self, ExitCode};
use std::rc::Rc;

use nifty_interp::RuntimeError;

use crate::frontend::SourceFile;
use crate::{CommandResult, EXIT_HAS_ERRORS, report};

pub(crate) fn run(target: &Path) -> CommandResult {
    let source = Rc::new(SourceFile::read(target)?);
    let program = match source.compile() {
        Ok(program) => program,
        Err(diagnostics) => {
            report::print(&source, &diagnostics)?;
            return Ok(ExitCode::from(EXIT_HAS_ERRORS));
        }
    };

    // An error inside an event leaves the app half-way through a change, so it stops there.
    let on_error = Rc::new({
        let source = Rc::clone(&source);
        move |error: RuntimeError| {
            report::runtime_error(&source, &error);
            process::exit(i32::from(EXIT_HAS_ERRORS));
        }
    });
    match nifty_interp::run(program, nifty_platform::platform_backend(), on_error) {
        Ok(()) => Ok(ExitCode::SUCCESS),
        Err(error) => {
            report::runtime_error(&source, &error);
            Ok(ExitCode::from(EXIT_HAS_ERRORS))
        }
    }
}
