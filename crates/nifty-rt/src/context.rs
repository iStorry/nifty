//! What compiled code runs against: the app's state, what changed since the UI last caught
//! up, how deep calls are, and why the code last failed.

use std::cell::{Cell, RefCell};
use std::fmt;
use std::mem;

use nifty_runtime::{DownloadCommand, Failure, FileDialog, StateId, StateSet, WindowRequest};

use crate::dialog::Answer;
use crate::download::Request;
use crate::ffi::Str;
use crate::program::ProgramDesc;
use crate::value::{Slot, release};

/// A failure while the app runs, pointing at the code responsible when the program says
/// where that is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppError {
    pub message: String,
    pub location: Option<SourceLocation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceLocation {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

impl AppError {
    pub(crate) fn malformed(why: &str) -> Self {
        Self {
            message: format!("this app was built wrong, so it can't run: {why}"),
            location: None,
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.location {
            Some(SourceLocation { file, line, column }) => {
                write!(f, "{file}:{line}:{column}: {}", self.message)
            }
            None => f.write_str(&self.message),
        }
    }
}

/// Compiled code receives this as an opaque pointer and passes it back on every call.
pub struct Context {
    program: &'static ProgramDesc,
    states: Box<[Cell<Slot>]>,
    /// States changed since the UI last caught up.
    changed: RefCell<StateSet>,
    call_depth: Cell<usize>,
    /// Why the code that just returned `false` failed, and the span id of where.
    failed: RefCell<Option<(Failure, u32)>>,
    /// Downloads asked for since the app last started them.
    downloads: RefCell<Vec<Request>>,
    /// What code asked of running downloads since the app last passed it on.
    download_commands: RefCell<Vec<(DownloadCommand, String)>>,
    /// File dialogs asked for since the app last passed them on.
    dialogs: RefCell<Vec<(FileDialog, Answer)>>,
    /// Windows asked to open or close since the app last passed them on.
    window_requests: RefCell<Vec<WindowRequest>>,
}

impl Context {
    /// Sets every state to its starting value, in order, so each may read the ones before.
    pub(crate) fn new(program: &'static ProgramDesc) -> Result<Self, AppError> {
        let states = program.states.items();
        let context = Self {
            program,
            states: states.iter().map(|_| Cell::new(0)).collect(),
            changed: RefCell::new(StateSet::default()),
            call_depth: Cell::new(0),
            failed: RefCell::new(None),
            downloads: RefCell::new(Vec::new()),
            download_commands: RefCell::new(Vec::new()),
            dialogs: RefCell::new(Vec::new()),
            window_requests: RefCell::new(Vec::new()),
        };
        for (slot, state) in context.states.iter().zip(states) {
            let mut value = 0;
            // SAFETY: compiled initialisers take the context and write their value to `out`.
            if !unsafe { (state.init)(&raw const context, &raw mut value) } {
                return Err(context.take_error());
            }
            slot.set(value);
        }
        Ok(context)
    }

    pub(crate) fn state(&self, state: usize) -> Slot {
        self.states[state].get()
    }

    /// The slot of `state`, which is marked changed since the caller is about to write it.
    pub(crate) fn state_mut(&self, state: usize) -> *mut Slot {
        self.changed.borrow_mut().insert(StateId(state));
        self.states[state].as_ptr()
    }

    pub(crate) fn take_changed(&self) -> StateSet {
        mem::take(&mut self.changed.borrow_mut())
    }

    pub(crate) fn fail(&self, failure: Failure, span: u32) {
        *self.failed.borrow_mut() = Some((failure, span));
    }

    /// Counts a call into `function`, or fails when calls are too deep.
    pub(crate) fn enter(&self, function: u32, span: u32) -> bool {
        let depth = self.call_depth.get();
        if depth >= nifty_runtime::MAX_CALL_DEPTH {
            let name = usize::try_from(function)
                .ok()
                .and_then(|function| self.program.functions.items().get(function))
                .map_or_else(|| format!("function {function}"), Str::lossy);
            self.fail(Failure::TooDeep { function: name }, span);
            return false;
        }
        self.call_depth.set(depth + 1);
        true
    }

    pub(crate) fn leave(&self) {
        self.call_depth.set(self.call_depth.get().saturating_sub(1));
    }

    pub(crate) fn request_download(&self, request: Request) {
        self.downloads.borrow_mut().push(request);
    }

    /// The downloads asked for since the last call.
    pub(crate) fn take_downloads(&self) -> Vec<Request> {
        mem::take(&mut self.downloads.borrow_mut())
    }

    pub(crate) fn request_download_command(&self, command: DownloadCommand, id: String) {
        self.download_commands.borrow_mut().push((command, id));
    }

    /// What code asked of running downloads since the last call, in order, by their ids.
    pub(crate) fn take_download_commands(&self) -> Vec<(DownloadCommand, String)> {
        mem::take(&mut self.download_commands.borrow_mut())
    }

    pub(crate) fn ask_dialog(&self, dialog: FileDialog, answer: Answer) {
        self.dialogs.borrow_mut().push((dialog, answer));
    }

    pub(crate) fn request_window(&self, request: WindowRequest) {
        self.window_requests.borrow_mut().push(request);
    }

    /// The windows asked to open or close since the last call, in order.
    pub(crate) fn take_window_requests(&self) -> Vec<WindowRequest> {
        mem::take(&mut self.window_requests.borrow_mut())
    }

    /// The file dialogs asked for since the last call.
    pub(crate) fn take_dialogs(&self) -> Vec<(FileDialog, Answer)> {
        mem::take(&mut self.dialogs.borrow_mut())
    }

    /// Why compiled code just returned `false`, which it does only after a runtime call
    /// recorded a failure.
    ///
    /// # Panics
    /// When no failure was recorded, which only wrongly compiled code does.
    pub(crate) fn take_error(&self) -> AppError {
        let (failure, span) = self
            .failed
            .borrow_mut()
            .take()
            .expect("compiled code fails only after the runtime records why");
        self.error(&failure, span)
    }

    pub(crate) fn error(&self, failure: &Failure, span: u32) -> AppError {
        AppError {
            message: failure.to_string(),
            location: self.location(span),
        }
    }

    fn location(&self, span: u32) -> Option<SourceLocation> {
        let span = self
            .program
            .spans
            .items()
            .get(usize::try_from(span).ok()?)?;
        Some(SourceLocation {
            file: span.file.lossy(),
            line: span.line,
            column: span.column,
        })
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        for (slot, state) in self.states.iter().zip(self.program.states.items()) {
            release(slot.get(), state.ty);
        }
    }
}

impl fmt::Debug for Context {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Context")
            .field("states", &self.states.len())
            .field("call_depth", &self.call_depth.get())
            .finish_non_exhaustive()
    }
}
