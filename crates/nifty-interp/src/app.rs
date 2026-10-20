//! Runs a checked program's windows on the runtime's engine, with the interpreter giving
//! every dynamic part its value.

use std::rc::Rc;

use nifty_check::hir::Program;
use nifty_runtime::{Backend, Engine, Ui};

use crate::logic::Interpreter;
use crate::lower;
use crate::machine::{Machine, Result, RuntimeError};

pub type ErrorHandler = nifty_runtime::ErrorHandler<RuntimeError>;

/// A running app: its state, its windows, and what each widget shows.
pub struct App<B: Backend> {
    engine: Rc<Engine<Interpreter, B>>,
}

impl<B: Backend> std::fmt::Debug for App<B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("App").finish_non_exhaustive()
    }
}

impl<B: Backend> App<B> {
    /// Sets up state and shows every window.
    ///
    /// # Errors
    /// When evaluating a starting value or building the first UI fails.
    ///
    /// # Panics
    /// When `program` has errors; run only programs the checker accepted.
    pub fn start(program: Program, ui: Ui<B>, on_error: ErrorHandler) -> Result<Self> {
        let machine = Machine::new(Rc::new(program), None)?;
        let app = lower::app(machine.program());
        let engine = Engine::start(ui, app, on_error, |ui, engine| {
            Interpreter::new(machine, ui, engine)
        })?;
        Ok(Self { engine })
    }

    /// Starts `program` in this app's windows, in place of this app. Each state it still
    /// declares with the same name and type keeps its value; the others start over.
    ///
    /// # Errors
    /// When evaluating a starting value or building the new UI fails. Nothing has changed
    /// then, and this app keeps running.
    ///
    /// # Panics
    /// When `program` has errors; run only programs the checker accepted.
    pub fn reload(&self, program: Program) -> Result<Self> {
        let machine = Machine::new(Rc::new(program), Some(self.engine.logic().machine()))?;
        let app = lower::app(machine.program());
        let engine = self
            .engine
            .reload(app, |ui, engine| Interpreter::new(machine, ui, engine))?;
        Ok(Self { engine })
    }

    /// Runs the event loop until the app quits.
    pub fn run(&self) {
        self.engine.run();
    }

    #[cfg(test)]
    pub(crate) fn machine(&self) -> &Machine {
        self.engine.logic().machine()
    }

    /// Waits until every running download has sent its last report, without applying them.
    #[cfg(test)]
    pub(crate) fn wait_for_downloads(&self) {
        self.engine.logic().wait_for_downloads();
    }
}
