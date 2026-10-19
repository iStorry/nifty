//! The Nifty command-line tool: `nf`, and `nf-runner`, the program inside every app that
//! `nf build` makes.

mod build;
mod check;
mod create;
mod dev;
mod error;
mod frontend;
mod lsp;
mod project;
mod report;
mod run;
mod runner;
#[cfg(test)]
mod test_support;

use std::path::Path;
use std::process::ExitCode;

use error::CliError;

const USAGE: &str = "\
usage: nf <command> [args]

commands:
  create <name>      create a new app in a folder called <name>
  run [project]      run an app (default: the current folder)
  dev [project]      run an app and reload it each time you save, keeping its state
  check [project]    report errors in a project (default: the current folder)
  build [project] [--macos | --windows | --linux]
                     build a native app (default: for this system)
  lsp                run the language server on stdin and stdout, for editors
  version            print the version of nf

A project is a folder with a main.nf file, or a single .nf file.";

/// The program has errors, found while checking it or while it ran.
const EXIT_HAS_ERRORS: u8 = 1;
const EXIT_USAGE: u8 = 2;

/// Runs `nf` with its command-line arguments, without the program name.
#[must_use]
pub fn nf(args: &[&str]) -> ExitCode {
    let result = match args {
        ["create", name] => create::run(name),
        ["run"] => run::run(Path::new(".")),
        ["run", target] => run::run(Path::new(target)),
        ["dev"] => dev::run(Path::new(".")),
        ["dev", target] => dev::run(Path::new(target)),
        ["check"] => check::run(Path::new(".")),
        ["check", target] => check::run(Path::new(target)),
        ["build", args @ ..] => build::run(args),
        ["lsp"] => lsp::run(),
        ["help" | "--help" | "-h"] => {
            println!("{USAGE}");
            Ok(ExitCode::SUCCESS)
        }
        ["version" | "--version" | "-V"] => {
            println!("nf {}", env!("CARGO_PKG_VERSION"));
            Ok(ExitCode::SUCCESS)
        }
        _ => {
            eprintln!("{USAGE}");
            Ok(ExitCode::from(EXIT_USAGE))
        }
    };
    finish(result)
}

/// Runs the project bundled with the app this program is part of.
#[must_use]
pub fn run_bundled_app() -> ExitCode {
    finish(runner::run())
}

fn finish(result: CommandResult) -> ExitCode {
    result.unwrap_or_else(|error| {
        eprintln!("error: {error}");
        ExitCode::from(EXIT_USAGE)
    })
}

/// Result of a command: the exit code to return, or an error that stopped it.
type CommandResult = Result<ExitCode, CliError>;
