//! The methods only strings have, and `json_text`, in compiled apps. Every string they take
//! is borrowed, and every string or list they give is owned by the caller.

use nifty_runtime::Failure;

use crate::abi::borrow_context;
use crate::context::Context;
use crate::value::{self, Slot};

/// `text.contains(needle)`.
#[unsafe(no_mangle)]
pub(crate) extern "C" fn nf_text_contains(text: Slot, needle: Slot) -> bool {
    value::as_str(text).contains(value::as_str(needle))
}

/// `text.starts_with(prefix)`.
#[unsafe(no_mangle)]
pub(crate) extern "C" fn nf_text_starts_with(text: Slot, prefix: Slot) -> bool {
    value::as_str(text).starts_with(value::as_str(prefix))
}

/// `text.ends_with(suffix)`.
#[unsafe(no_mangle)]
pub(crate) extern "C" fn nf_text_ends_with(text: Slot, suffix: Slot) -> bool {
    value::as_str(text).ends_with(value::as_str(suffix))
}

/// `text.find(needle)`, written to `out`.
///
/// # Safety
/// See the `abi` module's contract.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn nf_text_find(
    context: *const Context,
    text: Slot,
    needle: Slot,
    span: u32,
    out: *mut i64,
) -> bool {
    let found = nifty_runtime::find(value::as_str(text), value::as_str(needle));
    // SAFETY: by the caller's contract.
    unsafe { give(context, found, span, out) }
}

/// `text.slice(start, end)`, written to `out` as an owned string.
///
/// # Safety
/// See the `abi` module's contract.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn nf_text_slice(
    context: *const Context,
    text: Slot,
    start: i64,
    end: i64,
    span: u32,
    out: *mut Slot,
) -> bool {
    let sliced = nifty_runtime::slice(value::as_str(text), start, end).map(value::string_slot);
    // SAFETY: by the caller's contract.
    unsafe { give(context, sliced, span, out) }
}

/// `text.split(separator)`.
#[unsafe(no_mangle)]
pub(crate) extern "C" fn nf_text_split(text: Slot, separator: Slot) -> Slot {
    value::strings_slot(nifty_runtime::split(
        value::as_str(text),
        value::as_str(separator),
    ))
}

/// `text.replace(from, to)`.
#[unsafe(no_mangle)]
pub(crate) extern "C" fn nf_text_replace(text: Slot, from: Slot, to: Slot) -> Slot {
    value::string_slot(nifty_runtime::replace(
        value::as_str(text),
        value::as_str(from),
        value::as_str(to),
    ))
}

/// `text.trim()`.
#[unsafe(no_mangle)]
pub(crate) extern "C" fn nf_text_trim(text: Slot) -> Slot {
    value::string_slot(value::as_str(text).trim())
}

/// `json_text(json, path)`.
#[unsafe(no_mangle)]
pub(crate) extern "C" fn nf_json_text(json: Slot, path: Slot) -> Slot {
    let path: Vec<&str> = value::as_list(path)
        .items
        .iter()
        .map(|step| value::as_str(*step))
        .collect();
    value::string_slot(nifty_runtime::json_text(value::as_str(json), &path))
}

/// Writes a call's result to `out`, or records why it failed.
///
/// # Safety
/// See the `abi` module's contract.
unsafe fn give<T>(
    context: *const Context,
    result: Result<T, Failure>,
    span: u32,
    out: *mut T,
) -> bool {
    match result {
        // SAFETY: by the caller's contract.
        Ok(value) => unsafe {
            out.write(value);
            true
        },
        Err(failure) => {
            // SAFETY: by the caller's contract.
            unsafe { borrow_context(context) }.fail(failure, span);
            false
        }
    }
}
