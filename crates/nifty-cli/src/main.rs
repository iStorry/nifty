//! The `nifty` command-line tool.

mod report;

use std::path::Path;
use std::process::ExitCode;
use std::{env, fs};

const USAGE: &str = "\
usage: nifty <command> <file.nifty>

commands:
  check    report syntax errors in a nifty source file";

const EXIT_HAS_ERRORS: u8 = 1;
const EXIT_USAGE: u8 = 2;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.as_slice() {
        [command, path] if command == "check" => check(Path::new(path)),
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(EXIT_USAGE)
        }
    }
}

fn check(path: &Path) -> ExitCode {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("error: can't read {}: {error}", path.display());
            return ExitCode::from(EXIT_USAGE);
        }
    };

    let parsed = nifty_syntax::parse(&source);
    if parsed.diagnostics.is_empty() {
        println!("{}: no problems found", path.display());
        return ExitCode::SUCCESS;
    }

    let file_name = path.display().to_string();
    if let Err(error) = report::print(&file_name, &source, &parsed.diagnostics) {
        eprintln!("error: can't write diagnostics: {error}");
        return ExitCode::from(EXIT_USAGE);
    }
    ExitCode::from(EXIT_HAS_ERRORS)
}
