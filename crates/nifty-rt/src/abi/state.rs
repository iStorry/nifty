//! The app's state, calls between functions, and failures compiled code detects itself.

use nifty_runtime::Failure;

use super::borrow_context;
use crate::context::Context;
use crate::value::Slot;

/// The value of state `state`, borrowed from the app.
///
/// # Safety
/// See the module's contract.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn nf_state(context: *const Context, state: u32) -> Slot {
    // SAFETY: by the caller's contract.
    unsafe { borrow_context(context) }.state(state as usize)
}

/// The slot of state `state`, marked changed, for compiled code to write.
///
/// # Safety
/// See the module's contract.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn nf_state_mut(context: *const Context, state: u32) -> *mut Slot {
    // SAFETY: by the caller's contract.
    unsafe { borrow_context(context) }.state_mut(state as usize)
}

/// Counts a call into function `function` from the call at `span`, failing when calls
/// are too deep. Each successful call is matched by `nf_leave`.
///
/// # Safety
/// See the module's contract.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn nf_enter(
    context: *const Context,
    function: u32,
    span: u32,
) -> bool {
    // SAFETY: by the caller's contract.
    unsafe { borrow_context(context) }.enter(function, span)
}

/// # Safety
/// See the module's contract.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn nf_leave(context: *const Context) {
    // SAFETY: by the caller's contract.
    unsafe { borrow_context(context) }.leave();
}

/// # Safety
/// See the module's contract.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn nf_fail_divide_by_zero(context: *const Context, span: u32) {
    // SAFETY: by the caller's contract.
    unsafe { borrow_context(context) }.fail(Failure::DivideByZero, span);
}

/// # Safety
/// See the module's contract.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn nf_fail_too_large(context: *const Context, span: u32) {
    // SAFETY: by the caller's contract.
    unsafe { borrow_context(context) }.fail(Failure::TooLarge, span);
}
