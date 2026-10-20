//! `open_window(…)` and `close_window(…)` in compiled apps, which ask for the window by
//! its position among the app's windows.

use nifty_runtime::{WindowAction, WindowRequest};

use crate::abi::borrow_context;
use crate::context::Context;

/// Asks for `action` on the app's window at position `window`, done once the current
/// event finishes.
///
/// # Safety
/// See the `abi` module's contract; `window` is the position of one of the program's
/// windows.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn nf_window(
    context: *const Context,
    action: WindowAction,
    window: u32,
) {
    // SAFETY: by the caller's contract.
    let context = unsafe { borrow_context(context) };
    context.request_window(WindowRequest {
        action,
        window: usize::try_from(window).expect("window positions fit in usize"),
    });
}
