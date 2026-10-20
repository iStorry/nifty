//! Runs a checked Nifty program directly, driving the native UI through the runtime's
//! engine. This is what `nf run` and `nf dev` use; `nf build` will compile the same program
//! to machine code that drives the same engine instead.

mod app;
mod download;
mod live;
mod logic;
mod lower;
mod machine;
mod value;

use nifty_check::hir::Program;
use nifty_runtime::{Backend, Ui};

pub use app::{App, ErrorHandler};
pub use live::LiveApp;
pub use machine::RuntimeError;
pub use value::Value;

/// Shows the program's windows on `backend` and runs until the app quits. Errors during
/// events go to `on_error`.
///
/// # Errors
/// When setting up state or building the first UI fails.
pub fn run<B: Backend>(
    program: Program,
    backend: B,
    on_error: ErrorHandler,
) -> Result<(), RuntimeError> {
    let app = App::start(program, Ui::new(backend), on_error)?;
    app.run();
    Ok(())
}

#[cfg(test)]
mod tests;
