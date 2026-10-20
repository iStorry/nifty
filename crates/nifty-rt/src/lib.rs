//! The runtime library compiled Nifty apps link against.
//!
//! `nf build` compiles a program to machine code that calls the `nf_*` functions in
//! [`abi`] and emits a [`ProgramDesc`] describing its states, windows and views. The
//! program's `main` hands that descriptor to [`nifty_rt_main`], which shows the windows on
//! the platform's native UI through the same engine the interpreter uses.

mod abi;
mod context;
mod ffi;
mod logic;
mod lower;
mod program;
mod show;
mod value;

use std::process;
use std::rc::Rc;

use nifty_runtime::{Backend, Engine, ErrorHandler, Ui};

pub use context::{AppError, Context, SourceLocation};
pub use ffi::{Slice, Str};
pub use logic::{Binding, Compiled, Frame, Region};
pub use program::{
    ABI_VERSION, ActionFn, BindingDesc, ExpandFn, FrameDesc, InitFn, ProgramDesc, ReadFn,
    RegionDesc, SpanDesc, StateDesc, TemplateDesc, TemplateKind, ViewDesc, WindowDesc, WriteFn,
};
pub use show::Sink;
pub use value::{BOOL, FLOAT, INT, STRING, Slot, TypeDesc, TypeKind};

/// The exit status of an app that stopped with an error.
const EXIT_FAILED: i32 = 1;

/// Sets up `program`'s state and shows its windows on `ui`. Errors during events go to
/// `on_error`.
///
/// # Errors
/// When the descriptor is malformed, or setting up state or building the first UI fails.
pub fn start<B: Backend>(
    program: &'static ProgramDesc,
    ui: Ui<B>,
    on_error: ErrorHandler<AppError>,
) -> Result<Rc<Engine<Compiled, B>>, AppError> {
    let lower::Lowered { windows, views } = lower::lower(program)?;
    let context = Context::new(program)?;
    Engine::start(ui, windows, on_error, |_, _| Compiled::new(context, views))
}

/// Runs `program` until the app quits, and returns the process's exit status. An error
/// stops the app, since it leaves the app half-way through a change.
///
/// # Safety
/// `program` points to a descriptor `nf build` emitted with this runtime's
/// [`ABI_VERSION`], next to the compiled code it describes, and this runs on the main
/// thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nifty_rt_main(program: *const ProgramDesc) -> i32 {
    // SAFETY: by the caller's contract, the descriptor lives as long as the program.
    let Some(program) = (unsafe { program.as_ref() }) else {
        eprintln!("{}", AppError::malformed("its program is missing"));
        return EXIT_FAILED;
    };
    let on_error = Rc::new(|error: AppError| {
        eprintln!("{error}");
        process::exit(EXIT_FAILED);
    });
    match start(
        program,
        Ui::new(nifty_platform::platform_backend()),
        on_error,
    ) {
        Ok(engine) => {
            engine.run();
            0
        }
        Err(error) => {
            eprintln!("{error}");
            EXIT_FAILED
        }
    }
}

#[cfg(test)]
mod tests;
