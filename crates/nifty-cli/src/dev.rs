//! `nf dev`: run the app and reload it in place every time its source is saved. State the new
//! version still declares with the same name and type keeps its value.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;
use std::time::Duration;

use nifty_interp::{ErrorHandler, LiveApp, RuntimeError};
use nifty_platform::PlatformBackend;
use nifty_runtime::Waker;
use notify_debouncer_mini::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer};

use crate::error::CliError;
use crate::frontend::SourceFile;
use crate::project::{self, SOURCE_EXTENSION};
use crate::{CommandResult, report};

/// Editors often save in several steps; changes this close together reload once.
const SAVE_SETTLE_TIME: Duration = Duration::from_millis(100);

/// The source of the version showing now, which runtime errors point into.
type Showing = Rc<RefCell<Option<Rc<SourceFile>>>>;

pub(crate) fn run(target: &Path) -> CommandResult {
    let folder = watched_folder(&project::entry_file(target)?);
    let showing = Showing::default();
    let session = Rc::new(DevSession {
        target: target.to_path_buf(),
        live: LiveApp::new(
            nifty_platform::platform_backend(),
            report_and_continue(&showing),
        ),
        showing,
        last_text: RefCell::new(None),
    });
    session.reload();

    let waker = session.live.waker(Box::new({
        let session = Rc::downgrade(&session);
        move || {
            if let Some(session) = session.upgrade() {
                session.reload();
            }
        }
    }));
    let _watcher = watch(&folder, waker)?;
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
    /// The text last read, so saving without changes doesn't reload.
    last_text: RefCell<Option<String>>,
}

impl DevSession {
    /// Reads, checks and shows the latest source. When it has errors, they are reported
    /// and the version showing before stays.
    fn reload(&self) {
        let source = match SourceFile::read(&self.target) {
            Ok(source) => source,
            Err(error) => {
                eprintln!("error: {error}");
                return;
            }
        };
        if self.last_text.borrow().as_deref() == Some(source.text.as_str()) {
            return;
        }
        self.last_text.replace(Some(source.text.clone()));

        let was_showing = self.showing.borrow().is_some();
        if self.load(&source) {
            let verb = if was_showing { "reloaded" } else { "started" };
            println!("{}: {verb}", source.name());
            self.showing.replace(Some(Rc::new(source)));
        } else if was_showing {
            eprintln!("still showing the last version that worked; save a fix to reload");
        } else {
            eprintln!("save a fix to start the app");
        }
    }

    /// Checks and shows `source`, reporting any errors; returns whether it is showing now.
    fn load(&self, source: &SourceFile) -> bool {
        match source.compile() {
            Ok(program) => match self.live.load(program) {
                Ok(()) => true,
                Err(error) => {
                    report::runtime_error(source, &error);
                    false
                }
            },
            Err(diagnostics) => {
                if let Err(error) = report::print(source, &diagnostics) {
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
        Some(source) => report::runtime_error(source, &error),
        None => eprintln!("error: {}", error.message),
    })
}

/// The folder holding the entry file. Watching it, rather than the file, catches editors
/// that save by replacing the file.
fn watched_folder(entry: &Path) -> PathBuf {
    match entry.parent() {
        Some(folder) if !folder.as_os_str().is_empty() => folder.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

/// Wakes the UI thread whenever a source file in `folder` changes.
fn watch(folder: &Path, waker: impl Waker) -> Result<Debouncer<RecommendedWatcher>, CliError> {
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
                    if events.iter().any(|event| is_source(&event.path)) {
                        waker.wake();
                    }
                }
                Err(error) => eprintln!("error: watching for changes failed: {error}"),
            },
        )
        .map_err(watch_error)?;
    debouncer
        .watcher()
        .watch(folder, RecursiveMode::NonRecursive)
        .map_err(watch_error)?;
    Ok(debouncer)
}

fn is_source(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == SOURCE_EXTENSION)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bare_file_name_is_watched_in_the_current_folder() {
        assert_eq!(watched_folder(Path::new("main.nf")), PathBuf::from("."));
        assert_eq!(
            watched_folder(Path::new("apps/todos/main.nf")),
            PathBuf::from("apps/todos")
        );
    }

    #[test]
    fn only_nf_files_count_as_source() {
        assert!(is_source(Path::new("/app/main.nf")));
        assert!(!is_source(Path::new("/app/main.nf.swp")));
        assert!(!is_source(Path::new("/app/notes.txt")));
    }
}
