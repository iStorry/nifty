//! `nf-runner`, the program inside every app `nf build` makes: it runs the bundled project.
#![cfg_attr(windows, windows_subsystem = "windows")]

use std::process::ExitCode;

fn main() -> ExitCode {
    nifty_cli::run_bundled_app()
}
