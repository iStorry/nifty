//! The calls compiled code makes into the runtime, exported unmangled as `nf_*`.
//!
//! Ownership follows one rule: a call takes ownership of the values passed to it unless
//! its parameter is a borrowed `Slot`, documented as such, and gives ownership of the
//! values it returns, except getters, whose results are borrowed from their container.
//! A call that can fail returns `false` after recording why in the context; compiled code
//! then returns `false` itself, up to the runtime.
//!
//! Every pointer passed in must be valid for the access the call makes, every `Slot` must
//! hold a value of the type the call expects, and every `context` must be the one the
//! runtime passed into the compiled code making the call. `nf build` upholds this for the
//! code it emits, which `nifty_rt_main`'s caller vouches for.

mod lists;
mod state;
mod strings;
mod structs;
mod values;

use crate::context::Context;

/// # Safety
/// `context` is the context the runtime passed into the compiled code calling.
unsafe fn borrow_context<'a>(context: *const Context) -> &'a Context {
    // SAFETY: by the caller's contract.
    unsafe { &*context }
}

/// A list's length, or a string's, as an `int`. Allocations never exceed `isize::MAX`
/// bytes, so the length always fits.
fn count(len: usize) -> i64 {
    i64::try_from(len).unwrap_or(i64::MAX)
}

#[cfg(test)]
pub(crate) use lists::*;
#[cfg(test)]
pub(crate) use state::*;
#[cfg(test)]
pub(crate) use strings::*;
#[cfg(test)]
pub(crate) use structs::*;
#[cfg(test)]
pub(crate) use values::*;
