//! Native desktop apps built from each platform's own widgets: no web view, no custom renderer.

pub use nifty_runtime::{
    Align, Axis, Button, Effect, IntoText, Justify, Label, Signal, Stack, Text, View, Window,
    button, column, label, row,
};

#[cfg(not(target_os = "macos"))]
compile_error!("nifty has a macOS backend only so far; Windows and Linux backends are next");

/// Opens `window` with the current platform's native widgets and runs until the app quits.
#[cfg(target_os = "macos")]
pub fn run(window: Window) {
    nifty_runtime::run(nifty_macos::MacBackend::new(), window);
}
