//! An app whose program can be replaced while it runs, which is how `nf dev` reloads.

use std::cell::RefCell;
use std::rc::Rc;

use nifty_check::hir::Program;
use nifty_runtime::{Backend, Ui, WakeHandler};

use crate::app::{App, ErrorHandler};
use crate::machine::RuntimeError;

pub struct LiveApp<B: Backend> {
    ui: Ui<B>,
    on_error: ErrorHandler,
    /// `None` until a program first loads.
    app: RefCell<Option<Rc<App<B>>>>,
}

impl<B: Backend> LiveApp<B> {
    /// An app showing nothing yet; [`LiveApp::load`] gives it a program.
    pub fn new(backend: B, on_error: ErrorHandler) -> Self {
        Self {
            ui: Ui::new(backend),
            on_error,
            app: RefCell::new(None),
        }
    }

    /// Shows `program` in place of the version running now, keeping each state it still
    /// declares with the same name and type, or opens its windows if nothing runs yet.
    ///
    /// # Errors
    /// When evaluating a starting value or building the UI fails. Whatever was showing
    /// before keeps running.
    ///
    /// # Panics
    /// When `program` has errors; load only programs the checker accepted.
    pub fn load(&self, program: Program) -> Result<(), RuntimeError> {
        let next = match &*self.app.borrow() {
            Some(running) => running.reload(program)?,
            None => App::start(program, self.ui.clone(), Rc::clone(&self.on_error))?,
        };
        *self.app.borrow_mut() = Some(next);
        Ok(())
    }

    /// Returns something any thread can use to have `on_wake` run on the UI thread, where
    /// it may call [`LiveApp::load`].
    #[must_use = "dropping the waker means `on_wake` never runs"]
    pub fn waker(&self, on_wake: WakeHandler) -> B::Waker {
        self.ui.waker(on_wake)
    }

    /// Runs the event loop until the app quits.
    pub fn run(&self) {
        self.ui.run();
    }
}

impl<B: Backend> std::fmt::Debug for LiveApp<B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiveApp").finish_non_exhaustive()
    }
}
