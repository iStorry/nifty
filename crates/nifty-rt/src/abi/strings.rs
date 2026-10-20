//! Strings, and building interpolated strings.

use std::slice;

use super::count;
use crate::value::{self, Slot, TypeDesc};

/// A string with a copy of `len` bytes of UTF-8 at `bytes`.
///
/// # Safety
/// `bytes` points to `len` readable bytes.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn nf_string_new(bytes: *const u8, len: usize) -> Slot {
    // SAFETY: by the caller's contract.
    let bytes = unsafe { slice::from_raw_parts(bytes, len) };
    value::string_slot(String::from_utf8_lossy(bytes))
}

/// `a + b`, with both borrowed.
#[unsafe(no_mangle)]
pub(crate) extern "C" fn nf_string_concat(a: Slot, b: Slot) -> Slot {
    value::string_slot([value::as_str(a), value::as_str(b)].concat())
}

/// How many characters the borrowed string holds.
#[unsafe(no_mangle)]
pub(crate) extern "C" fn nf_string_len(text: Slot) -> i64 {
    count(value::as_str(text).chars().count())
}

/// Whether `a` sorts before (-1), with (0) or after (1) `b`, both borrowed.
#[unsafe(no_mangle)]
pub(crate) extern "C" fn nf_string_compare(a: Slot, b: Slot) -> i32 {
    match value::as_str(a).cmp(value::as_str(b)) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

/// Starts an interpolated string, which `nf_text_finish` turns into a string.
#[unsafe(no_mangle)]
pub(crate) extern "C" fn nf_text_new() -> *mut String {
    Box::into_raw(Box::default())
}

/// Appends `len` bytes of UTF-8 at `bytes`.
///
/// # Safety
/// `text` came from `nf_text_new` and isn't finished; `bytes` points to `len` bytes.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn nf_text_push_str(text: *mut String, bytes: *const u8, len: usize) {
    // SAFETY: by the caller's contract.
    let (text, bytes) = unsafe { (&mut *text, slice::from_raw_parts(bytes, len)) };
    text.push_str(&String::from_utf8_lossy(bytes));
}

/// Appends the borrowed value as it reads inside a string.
///
/// # Safety
/// `text` came from `nf_text_new` and isn't finished.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn nf_text_push(text: *mut String, slot: Slot, ty: &TypeDesc) {
    // SAFETY: by the caller's contract.
    value::write(unsafe { &mut *text }, slot, ty);
}

/// # Safety
/// `text` came from `nf_text_new` and isn't finished; it is finished after this.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn nf_text_finish(text: *mut String) -> Slot {
    // SAFETY: by the caller's contract, ownership of the builder passes back here.
    let text = unsafe { Box::from_raw(text) };
    value::string_slot(*text)
}
