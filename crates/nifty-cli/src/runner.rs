//! `nf-runner`: finds the project bundled with the app it is part of and runs it.

use std::env;

use crate::build;
use crate::error::CliError;
use crate::{CommandResult, run};

pub(crate) fn run() -> CommandResult {
    let executable = env::current_exe()
        .map_err(|error| CliError::new(format!("can't find this app's files: {error}")))?;
    let project = build::bundled_project(&executable).ok_or_else(|| {
        CliError::new(
            "nf-runner runs inside apps made by `nf build`; to run a project, use `nf run`",
        )
    })?;
    run::run(&project)
}
