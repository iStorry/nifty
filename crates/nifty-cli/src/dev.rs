//! `nf dev`: run the app and reload it in place every time one of its source files is saved.
//! State the new version still declares with the same name and type keeps its value.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;
use std::time::Duration;

use nifty_interp::{ErrorHandler, LiveApp, RuntimeError};
use nifty_platform::PlatformBackend;
use nifty_runtime::Waker;
use nifty_syntax::project;
use notify_debouncer_mini::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer};

use crate::error::CliError;
use crate::frontend::Sources;
use crate::{CommandResult, report};

/// Editors often save in several steps; changes this close together reload once.
const SAVE_SETTLE_TIME: Duration = Duration::from_millis(100);

/// The sources of the version showing now, which runtime errors point into.
type Showing = Rc<RefCell<Option<Rc<Sources>>>>;

pub(crate) fn run(target: &Path) -> CommandResult {
    let first = Sources::read(target)?;
    let (folder, mode) = watched(target);
    let showing = Showing::default();
    let session = Rc::new(DevSession {
        target: target.to_path_buf(),
        live: LiveApp::new(
            nifty_platform::platform_backend(),
            report_and_continue(&showing),
        ),
        showing,
        last_read: RefCell::new(None),
    });
    session.show(first);

    let waker = session.live.waker(Box::new({
        let session = Rc::downgrade(&session);
        move || {
            if let Some(session) = session.upgrade() {
                session.reload();
            }
        }
    }));
    let _watcher = watch(&folder, mode, waker)?;
    println!(
        "watching {} for changes; quit the app or press Ctrl-C to stop",
        folder.display()
    );
    session.live.run();
    Ok(ExitCode::SUCCESS)
}

struct DevSession {
    target: PathBuf,
    live: LiveApp<PlatformBackend>,
    showing: Showing,
    /// The sources last read, so saving without changes doesn't reload.
    last_read: RefCell<Option<Rc<Sources>>>,
}

impl DevSession {
    fn reload(&self) {
        match Sources::read(&self.target) {
            Ok(sources) => self.show(sources),
            Err(error) => eprintln!("error: {error}"),
        }
    }

    /// Checks and shows `sources` unless they are the ones read last. When they have
    /// errors, the errors are reported and the version showing before stays.
    fn show(&self, sources: Sources) {
        if self.last_read.borrow().as_deref() == Some(&sources) {
            return;
        }
        let sources = Rc::new(sources);
        self.last_read.replace(Some(Rc::clone(&sources)));

        let was_showing = self.showing.borrow().is_some();
        if self.load(&sources) {
            let verb = if was_showing { "reloaded" } else { "started" };
            println!("{}: {verb}", sources.name());
            self.showing.replace(Some(sources));
        } else if was_showing {
            eprintln!("still showing the last version that worked; save a fix to reload");
        } else {
            eprintln!("save a fix to start the app");
        }
    }

    /// Checks and shows `sources`, reporting any errors; returns whether it is showing now.
    fn load(&self, sources: &Sources) -> bool {
        match sources.compile() {
            Ok(program) => match self.live.load(program) {
                Ok(()) => true,
                Err(error) => {
                    report::runtime_error(sources, &error);
                    false
                }
            },
            Err(diagnostics) => {
                if let Err(error) = report::print(sources, &diagnostics) {
                    eprintln!("error: {error}");
                }
                false
            }
        }
    }
}

/// While developing, an error in an event is reported and the app keeps running, so the
/// next save can fix it.
fn report_and_continue(showing: &Showing) -> ErrorHandler {
    let showing = Rc::clone(showing);
    Rc::new(move |error: RuntimeError| match &*showing.borrow() {
        Some(sources) => report::runtime_error(sources, &error),
        None => eprintln!("error: {}", error.message),
    })
}

/// Where saves show up: a project's whole folder, where files can also be added or removed,
/// or the folder holding a single file. Watching a folder rather than files catches editors
/// that save by replacing the file.
fn watched(target: &Path) -> (PathBuf, RecursiveMode) {
    if target.is_dir() {
        return (target.to_path_buf(), RecursiveMode::Recursive);
    }
    let folder = match target.parent() {
        Some(folder) if !folder.as_os_str().is_empty() => folder.to_path_buf(),
        _ => PathBuf::from("."),
    };
    (folder, RecursiveMode::NonRecursive)
}

/// Wakes the UI thread whenever a source file in `folder` changes.
fn watch(
    folder: &Path,
    mode: RecursiveMode,
    waker: impl Waker,
) -> Result<Debouncer<RecommendedWatcher>, CliError> {
    let watch_error = |error| {
        CliError::new(format!(
            "can't watch {} for changes: {error}",
            folder.display()
        ))
    };
    let mut debouncer =
        new_debouncer(
            SAVE_SETTLE_TIME,
            move |result: DebounceEventResult| match result {
                Ok(events) => {
                    if events.iter().any(|event| project::is_source(&event.path)) {
                        waker.wake();
                    }
                }
                Err(error) => eprintln!("error: watching for changes failed: {error}"),
            },
        )
        .map_err(watch_error)?;
    debouncer
        .watcher()
        .watch(folder, mode)
        .map_err(watch_error)?;
    Ok(debouncer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::scratch_dir;

    #[test]
    fn a_single_file_is_watched_in_its_folder() {
        assert_eq!(
            watched(Path::new("demo.nf")),
            (PathBuf::from("."), RecursiveMode::NonRecursive)
        );
        assert_eq!(
            watched(Path::new("apps/demo.nf")),
            (PathBuf::from("apps"), RecursiveMode::NonRecursive)
        );
    }

    #[test]
    fn a_project_is_watched_with_all_its_subfolders() {
        let dir = scratch_dir("dev-watch-project");
        assert_eq!(watched(&dir), (dir.clone(), RecursiveMode::Recursive));
    }

    #[test]
    fn only_nf_files_count_as_source() {
        assert!(project::is_source(Path::new("/app/main.nf")));
        assert!(!project::is_source(Path::new("/app/main.nf.swp")));
        assert!(!project::is_source(Path::new("/app/notes.txt")));
    }
}
