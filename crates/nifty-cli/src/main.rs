//! `nf`, the Nifty command-line tool.

mod check;
mod create;
mod error;
mod project;
mod report;

use std::env;
use std::path::Path;
use std::process::ExitCode;

use error::CliError;

const USAGE: &str = "\
usage: nf <command> [args]

commands:
  create <name>      create a new app in a folder called <name>
  check [project]    report errors in a project (default: the current folder)

A project is a folder with a main.nf file, or a single .nf file.";

const EXIT_USAGE: u8 = 2;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        ["create", name] => create::run(name),
        ["check"] => check::run(Path::new(".")),
        ["check", target] => check::run(Path::new(target)),
        ["help" | "--help" | "-h"] => {
            println!("{USAGE}");
            Ok(ExitCode::SUCCESS)
        }
        _ => {
            eprintln!("{USAGE}");
            Ok(ExitCode::from(EXIT_USAGE))
        }
    };
    result.unwrap_or_else(|error| {
        eprintln!("error: {error}");
        ExitCode::from(EXIT_USAGE)
    })
}

/// Result of a command: the exit code to return, or an error that stopped it.
type CommandResult = Result<ExitCode, CliError>;
