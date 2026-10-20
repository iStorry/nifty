//! `nf build`: check the app, compile it to machine code, and package it as a native app
//! for an operating system.

mod macos;
mod windows;

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{self, ExitCode};
use std::sync::atomic::{AtomicUsize, Ordering};

use nifty_check::hir::Program;
use nifty_codegen::{RUNTIME_LIBRARY, compile_app, link};

use crate::error::CliError;
use crate::frontend::Sources;
use crate::{CommandResult, EXIT_HAS_ERRORS, report};

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
    // `nf` compiles for the system it runs on, and has only that system's runtime library.
    if request.target != Target::HOST {
        return Err(CliError::new(format!(
            "nf builds apps for the system it runs on, so build {0} apps on {0}",
            request.target.name()
        )));
    }
    if request.target == Target::Linux {
        return Err(CliError::new(
            "Nifty has no Linux backend yet, so it can't build Linux apps",
        ));
    }
    let sources = Sources::read(request.project)?;
    let program = match sources.compile() {
        Ok(program) => program,
        Err(diagnostics) => {
            report::print(&sources, &diagnostics)?;
            return Ok(ExitCode::from(EXIT_HAS_ERRORS));
        }
    };
    let name = app_name(request.project)?;
    let output = output_dir(&sources.entry().path);
    let app = build(&sources, &program, &name, &output, &runtime_library()?)?;
    println!("built {}", app.display());
    Ok(ExitCode::SUCCESS)
}

/// Compiles and links `program` in a scratch folder, then writes the app around the
/// executable, so a failure leaves an earlier build as it was. Returns the app.
fn build(
    sources: &Sources,
    program: &Program,
    name: &str,
    output: &Path,
    runtime: &Path,
) -> Result<PathBuf, CliError> {
    // Unique to this build, even with other builds running in this process or others.
    static BUILDS: AtomicUsize = AtomicUsize::new(0);
    let number = BUILDS.fetch_add(1, Ordering::Relaxed);
    let scratch = env::temp_dir().join(format!("nf-build-{}-{number}", process::id()));
    let app =
        link_in(&scratch, sources, program, name, runtime).and_then(
            |executable| match Target::HOST {
                Target::MacOs => macos::write_app(output, name, &executable),
                Target::Windows | Target::Linux => windows::write_app(output, name, &executable),
            },
        );
    let cleaned = fs::remove_dir_all(&scratch)
        .map_err(|error| CliError::new(format!("can't remove {}: {error}", scratch.display())));
    let app = app?;
    cleaned?;
    Ok(app)
}

/// Compiles and links `program` into an executable in `scratch`, and returns its path.
fn link_in(
    scratch: &Path,
    sources: &Sources,
    program: &Program,
    name: &str,
    runtime: &Path,
) -> Result<PathBuf, CliError> {
    let failed =
        |error: io::Error| CliError::new(format!("can't write {}: {error}", scratch.display()));
    fs::create_dir_all(scratch).map_err(failed)?;
    let compiled = compile_app(name, program, &|span| sources.locate(span))
        .map_err(|error| CliError::new(format!("can't compile {name}: {error}")))?;
    let object = scratch.join("app.o");
    fs::write(&object, compiled).map_err(failed)?;
    let executable = scratch.join(format!("app{}", env::consts::EXE_SUFFIX));
    link(&object, runtime, &executable).map_err(|error| CliError::new(error.to_string()))?;
    Ok(executable)
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

/// The runtime library every app is linked against, installed next to `nf`. Package
/// managers put a link to `nf` on the `PATH`, so the lookup starts from where that link
/// points.
fn runtime_library() -> Result<PathBuf, CliError> {
    let nf = env::current_exe()
        .and_then(fs::canonicalize)
        .map_err(|error| CliError::new(format!("can't find where nf is installed: {error}")))?;
    let library = nf.with_file_name(RUNTIME_LIBRARY);
    if library.is_file() {
        Ok(library)
    } else {
        Err(CliError::new(format!(
            "{} is missing; reinstall Nifty to get it back",
            library.display()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nifty_codegen::testing::runtime_library;

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

    /// A project in `dir` whose only file is `main.nf`, holding `source`.
    fn project(dir: &Path, source: &str) -> (PathBuf, Sources, Program) {
        let project = dir.join("project");
        fs::create_dir_all(&project).expect("create the project folder");
        fs::write(project.join("main.nf"), source).expect("write main.nf");
        let sources = Sources::read(&project).expect("read the project");
        let program = sources.compile().expect("the project checks");
        (project, sources, program)
    }

    /// The executable inside the app `nf build` wrote.
    fn executable(app: &Path, name: &str) -> PathBuf {
        match Target::HOST {
            Target::MacOs => app.join("Contents/MacOS").join(name),
            Target::Windows | Target::Linux => app.join(format!("{name}.exe")),
        }
    }

    #[test]
    fn builds_the_counter_example() {
        let dir = scratch_dir("build-counter");
        let source = include_str!("../../../fixtures/counter.nf");
        let (_, sources, program) = project(&dir, source);

        let app = build(
            &sources,
            &program,
            "Counter",
            &dir.join("build"),
            &runtime_library(),
        )
        .unwrap_or_else(|error| panic!("{error}"));

        assert!(executable(&app, "Counter").is_file());
    }

    #[test]
    fn built_apps_run_compiled_and_point_errors_at_the_source() {
        let dir = scratch_dir("build-failing");
        let source = "fn zero() -> int {\n  return 0\n}\n\nstate count = 1 / zero()\n\nwindow(\"Never\") {}\n";
        let (_, sources, program) = project(&dir, source);

        let app = build(
            &sources,
            &program,
            "broken",
            &dir.join("build"),
            &runtime_library(),
        )
        .unwrap_or_else(|error| panic!("{error}"));

        let output = std::process::Command::new(executable(&app, "broken"))
            .output()
            .expect("run the app");
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(
            String::from_utf8_lossy(&output.stderr).trim_end(),
            "main.nf:5:15: can't divide by zero"
        );
    }
}
