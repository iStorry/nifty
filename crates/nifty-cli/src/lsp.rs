//! `nf lsp`: the language server editors start to check and navigate nifty code.

use std::process::ExitCode;

use crate::CommandResult;
use crate::error::CliError;

pub(crate) fn run() -> CommandResult {
    nifty_lsp::run().map_err(|error| CliError::new(format!("language server stopped: {error}")))?;
    Ok(ExitCode::SUCCESS)
}
