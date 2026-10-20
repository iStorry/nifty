//! The runtime library compiled Nifty apps link against.
//!
//! `nf build` compiles a program to machine code that calls the `nf_*` functions
//! [`symbols`] names, and emits a [`ProgramDesc`] describing its states, windows and
//! views. The program's `main` hands that descriptor to [`nifty_rt_main`], which shows the
//! windows on the platform's native UI through the same engine the interpreter uses.

mod abi;
mod context;
mod dialog;
mod download;
mod ffi;
mod logic;
mod lower;
mod place;
mod program;
mod show;
pub mod symbols;
mod value;

use std::process;
use std::rc::Rc;

use nifty_runtime::{AppTemplate, Backend, Engine, ErrorHandler, Ui};

pub use context::{AppError, Context, SourceLocation};
pub use ffi::{Slice, Str};
pub use logic::{Binding, Compiled, Frame, Region};
pub use place::{PathStep, PlaceDesc, StepKind};
pub use program::{
    ABI_VERSION, ActionFn, BindingDesc, ExpandFn, FrameDesc, InitFn, MenuDesc, MenuEntryDesc,
    MenuEntryKind, ProgramDesc, ReadFn, RegionDesc, ShortcutDesc, SpanDesc, StateDesc,
    TemplateDesc, TemplateKind, ViewDesc, WindowDesc, WriteFn,
};
pub use show::Sink;
pub use value::{BOOL, FLOAT, INT, STRING, Slot, TypeDesc, TypeKind};

/// The exit status of an app that stopped with an error.
const EXIT_FAILED: i32 = 1;

/// The system libraries an app linked with this runtime needs, in the linker's own
/// syntax, as `rustc --print native-static-libs` reports them for the static library,
/// less `-lSystem`, which the C compiler driver always links. `nf build`'s link test
/// fails when one is missing.
#[cfg(target_os = "macos")]
pub const NATIVE_LIBS: &[&str] = &[
    "-lc",
    "-lm",
    "-liconv",
    "-lobjc",
    "-framework",
    "AppKit",
    "-framework",
    "CloudKit",
    "-framework",
    "CoreData",
    "-framework",
    "CoreFoundation",
    "-framework",
    "CoreGraphics",
    "-framework",
    "CoreImage",
    "-framework",
    "CoreText",
    "-framework",
    "CoreVideo",
    "-framework",
    "Foundation",
    "-framework",
    "QuartzCore",
    "-framework",
    "Security",
];

/// See the macOS list; Windows apps also link the C runtime statically, as `nf` does.
#[cfg(windows)]
pub const NATIVE_LIBS: &[&str] = &[
    "kernel32.lib",
    "ntdll.lib",
    "userenv.lib",
    "ws2_32.lib",
    "dbghelp.lib",
    "/defaultlib:libcmt",
];

/// A program whose descriptor checked out and whose state is set up, ready to show.
#[derive(Debug)]
pub struct Prepared {
    app: AppTemplate<Compiled>,
    views: Vec<logic::ViewTemplate>,
    context: Context,
}

/// Checks `program`'s descriptor and sets up its state, before any UI exists.
///
/// # Errors
/// When the descriptor is malformed, or setting up state fails.
pub fn prepare(program: &'static ProgramDesc) -> Result<Prepared, AppError> {
    let lower::Lowered { app, views } = lower::lower(program)?;
    let context = Context::new(program)?;
    Ok(Prepared {
        app,
        views,
        context,
    })
}

impl Prepared {
    /// Shows the program's windows on `ui`. Errors during events go to `on_error`.
    ///
    /// # Errors
    /// When building the first UI fails.
    pub fn show<B: Backend>(
        self,
        ui: Ui<B>,
        on_error: ErrorHandler<AppError>,
    ) -> Result<Rc<Engine<Compiled, B>>, AppError> {
        let Self {
            app,
            views,
            context,
        } = self;
        Engine::start(ui, app, on_error, |ui, engine| {
            Compiled::new(context, views, ui, engine)
        })
    }
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
    let shown = prepare(program)
        .and_then(|prepared| prepared.show(Ui::new(nifty_platform::platform_backend()), on_error));
    match shown {
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
