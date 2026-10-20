//! File dialogs, shared by the interpreter and compiled apps. Code asks for one during an
//! event; it is shown once the event has finished and the UI caught up, and the user's
//! answer comes back through the app's waker, like a download's reports.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt;
use std::mem;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use crate::backend::{Backend, Waker};
use crate::paths::{NoHomeFolder, expand_home};

/// What a dialog asks the user to pick. Compiled code passes it to the runtime as a `u32`,
/// which every platform's C calling convention passes without widening.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum DialogKind {
    /// One existing file.
    File,
    /// Any number of existing files, at least one.
    Files,
    /// One existing folder.
    Folder,
    /// Where to save a new file, which may already exist.
    SavePath,
}

impl DialogKind {
    /// Whether the answer is a list of paths rather than one.
    #[must_use]
    pub fn picks_many(self) -> bool {
        self == Self::Files
    }
}

/// A dialog as the platform shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileDialog {
    pub kind: DialogKind,
    /// Shown above the files; empty for the platform's own wording.
    pub title: String,
    /// The file extensions allowed, without the dot, such as `png`; empty allows any.
    pub types: Vec<String>,
    /// The folder it opens in, which exists; `None` lets the platform choose.
    pub folder: Option<PathBuf>,
    /// The file name a save dialog suggests; empty for none.
    pub name: String,
}

impl FileDialog {
    /// The dialog an app's call asks for. `types` may be written with or without the dot.
    /// `folder` is written as in the app, `~` included; it is left to the platform when
    /// empty or not an existing folder.
    ///
    /// # Errors
    /// When `folder` starts with `~` and the home folder can't be found.
    pub fn new(
        kind: DialogKind,
        title: String,
        types: &[&str],
        folder: &str,
        name: String,
    ) -> Result<Self, NoHomeFolder> {
        let folder = if folder.is_empty() {
            None
        } else {
            Some(expand_home(folder)?).filter(|folder| folder.is_dir())
        };
        Ok(Self {
            kind,
            title,
            types: types
                .iter()
                .map(|extension| extension.trim_start_matches('.'))
                .filter(|extension| !extension.is_empty())
                .map(str::to_owned)
                .collect(),
            folder,
            name,
        })
    }
}

/// How a dialog closed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Picked {
    /// The paths the user chose: exactly one, except for [`DialogKind::Files`].
    Chosen(Vec<String>),
    Cancelled,
    /// The platform couldn't show the dialog or read its answer.
    Failed(String),
}

pub type DialogHandler = Box<dyn FnOnce(Picked)>;

/// Shows file dialogs; every [`Backend`] is one, so code that isn't generic over the
/// backend can hold it as `Rc<dyn ShowDialog>`.
pub trait ShowDialog {
    fn show_dialog(&self, dialog: &FileDialog, on_close: DialogHandler);
}

impl<B: Backend> ShowDialog for B {
    fn show_dialog(&self, dialog: &FileDialog, on_close: DialogHandler) {
        self.show_file_dialog(dialog, on_close);
    }
}

/// The dialogs an app has asked for, and the answers that have come back. `T` says where
/// each answer goes, in whatever terms the app uses.
pub struct Dialogs<T> {
    shower: Rc<dyn ShowDialog>,
    waker: Arc<dyn Waker>,
    asked: RefCell<Vec<(FileDialog, T)>>,
    answers: Rc<RefCell<VecDeque<(T, Picked)>>>,
}

impl<T: 'static> Dialogs<T> {
    /// `waker` asks the UI thread to call [`Dialogs::show_asked`] and then, inside an event,
    /// [`Dialogs::receive`].
    pub fn new(shower: Rc<dyn ShowDialog>, waker: Arc<dyn Waker>) -> Self {
        Self {
            shower,
            waker,
            asked: RefCell::new(Vec::new()),
            answers: Rc::new(RefCell::new(VecDeque::new())),
        }
    }

    /// Asks for `dialog`, to be shown once the current event has finished.
    pub fn ask(&self, dialog: FileDialog, target: T) {
        self.asked.borrow_mut().push((dialog, target));
        self.waker.wake();
    }

    /// Shows every dialog asked for since the last call. Call it from the waker's handler
    /// outside any event: a platform may run a modal loop here, which handles other events.
    pub fn show_asked(&self) {
        let asked = mem::take(&mut *self.asked.borrow_mut());
        for (dialog, target) in asked {
            let answers = Rc::clone(&self.answers);
            let waker = Arc::clone(&self.waker);
            self.shower.show_dialog(
                &dialog,
                Box::new(move |picked| {
                    answers.borrow_mut().push_back((target, picked));
                    waker.wake();
                }),
            );
        }
    }

    /// Hands each answer that has come back to `apply` with where it goes. Stops at the
    /// first error, leaving later answers queued.
    ///
    /// # Errors
    /// The first error `apply` returns.
    pub fn receive<E>(&self, mut apply: impl FnMut(&T, Picked) -> Result<(), E>) -> Result<(), E> {
        loop {
            let next = self.answers.borrow_mut().pop_front();
            let Some((target, picked)) = next else {
                return Ok(());
            };
            apply(&target, picked)?;
        }
    }
}

impl<T> fmt::Debug for Dialogs<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Dialogs")
            .field("asked", &self.asked.borrow().len())
            .field("answers", &self.answers.borrow().len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
