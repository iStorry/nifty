//! `nf build`: check the app, then package it as a native app for an operating system.

pub(crate) mod macos;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::error::CliError;
use crate::frontend::SourceFile;
use crate::{CommandResult, EXIT_HAS_ERRORS, report};

/// Must match the `nf-runner` binary in this crate's `Cargo.toml`, which installs it next
/// to `nf`.
const RUNNER: &str = "nf-runner";
/// Built apps go in this folder, next to the project's entry file.
const OUTPUT_DIR: &str = "build";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    MacOs,
    Windows,
    Linux,
}

impl Target {
    const ALL: [Self; 3] = [Self::MacOs, Self::Windows, Self::Linux];

    /// The system `nf` itself was built for.
    const HOST: Self = if cfg!(target_os = "macos") {
        Self::MacOs
    } else if cfg!(target_os = "windows") {
        Self::Windows
    } else {
        Self::Linux
    };

    fn flag(self) -> &'static str {
        match self {
            Self::MacOs => "--macos",
            Self::Windows => "--windows",
            Self::Linux => "--linux",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::MacOs => "macOS",
            Self::Windows => "Windows",
            Self::Linux => "Linux",
        }
    }

    fn from_flag(flag: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|target| target.flag() == flag)
    }
}

#[derive(Debug, PartialEq)]
struct Request<'a> {
    project: &'a Path,
    target: Target,
}

pub(crate) fn run(args: &[&str]) -> CommandResult {
    let request = parse(args)?;
    let source = SourceFile::read(request.project)?;
    if let Err(diagnostics) = source.compile() {
        report::print(&source, &diagnostics)?;
        return Ok(ExitCode::from(EXIT_HAS_ERRORS));
    }

    let name = app_name(request.project)?;
    let output = output_dir(&source.path);
    let app = match request.target {
        Target::MacOs => macos::write_app(&output, &name, &source.text, &runner()?)?,
        target @ (Target::Windows | Target::Linux) => {
            return Err(CliError::new(format!(
                "Nifty has no {0} backend yet, so it can't build {0} apps",
                target.name()
            )));
        }
    };
    println!("built {}", app.display());
    Ok(ExitCode::SUCCESS)
}

fn parse<'a>(args: &[&'a str]) -> Result<Request<'a>, CliError> {
    let mut project = None;
    let mut target = None;
    for arg in args {
        if arg.starts_with('-') {
            let flags = Target::ALL.map(Target::flag).join(", ");
            let chosen = Target::from_flag(arg)
                .ok_or_else(|| CliError::new(format!("unknown option `{arg}`; use {flags}")))?;
            if target.replace(chosen).is_some() {
                return Err(CliError::new(format!(
                    "build for one system at a time: {flags}"
                )));
            }
        } else if project.replace(Path::new(*arg)).is_some() {
            return Err(CliError::new("build one project at a time"));
        }
    }
    Ok(Request {
        project: project.unwrap_or(Path::new(".")),
        target: target.unwrap_or(Target::HOST),
    })
}

/// A project folder's name, or a single file's name without `.nf`.
fn app_name(project: &Path) -> Result<String, CliError> {
    let path = fs::canonicalize(project)
        .map_err(|error| CliError::new(format!("can't find {}: {error}", project.display())))?;
    let name = if path.is_dir() {
        path.file_name()
    } else {
        path.file_stem()
    };
    name.and_then(|name| name.to_str())
        .map(str::to_owned)
        .ok_or_else(|| {
            CliError::new(format!(
                "can't name an app after {}; give the project folder a plain name",
                path.display()
            ))
        })
}

fn output_dir(entry: &Path) -> PathBuf {
    entry.parent().unwrap_or(Path::new("")).join(OUTPUT_DIR)
}

/// The runner every built app contains, installed next to `nf`.
fn runner() -> Result<PathBuf, CliError> {
    let nf = env::current_exe()
        .map_err(|error| CliError::new(format!("can't find where nf is installed: {error}")))?;
    let runner = nf.with_file_name(format!("{RUNNER}{}", env::consts::EXE_SUFFIX));
    if runner.is_file() {
        Ok(runner)
    } else {
        Err(CliError::new(format!(
            "{} is missing; reinstall nf with `cargo install --path crates/nifty-cli`",
            runner.display()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::scratch_dir;

    #[test]
    fn builds_the_current_folder_for_this_system_by_default() {
        assert_eq!(
            parse(&[]).expect("no arguments is fine"),
            Request {
                project: Path::new("."),
                target: Target::HOST
            }
        );
    }

    #[test]
    fn takes_a_project_and_a_target_in_any_order() {
        let expected = Request {
            project: Path::new("todos"),
            target: Target::Windows,
        };
        assert_eq!(parse(&["todos", "--windows"]).expect("valid"), expected);
        assert_eq!(parse(&["--windows", "todos"]).expect("valid"), expected);
    }

    #[test]
    fn rejects_unknown_repeated_and_conflicting_arguments() {
        for args in [
            &["--win"][..],
            &["--macos", "--linux"],
            &["--macos", "--macos"],
            &["a", "b"],
        ] {
            assert!(parse(args).is_err(), "{args:?} should be rejected");
        }
    }

    #[test]
    fn apps_are_named_after_their_folder_or_file() {
        let dir = scratch_dir("build-name");
        let folder = dir.join("todos");
        fs::create_dir(&folder).expect("create project folder");
        let file = dir.join("demo.nf");
        fs::write(&file, "").expect("write demo.nf");

        assert_eq!(app_name(&folder).expect("a folder"), "todos");
        assert_eq!(app_name(&file).expect("a file"), "demo");
    }

    #[test]
    fn apps_are_built_next_to_the_entry_file() {
        assert_eq!(
            output_dir(Path::new("todos/main.nf")),
            Path::new("todos/build")
        );
        assert_eq!(output_dir(Path::new("main.nf")), Path::new("build"));
    }
}
