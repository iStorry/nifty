//! Owning, comparing and storing values of any type.

use crate::value::{self, Slot, TypeDesc};

/// Adds an owner to `slot`, which is borrowed.
#[unsafe(no_mangle)]
pub(crate) extern "C" fn nf_retain(slot: Slot, ty: &TypeDesc) {
    value::retain(slot, ty);
}

#[unsafe(no_mangle)]
pub(crate) extern "C" fn nf_release(slot: Slot, ty: &TypeDesc) {
    value::release(slot, ty);
}

/// `a == b`, with both borrowed.
#[unsafe(no_mangle)]
pub(crate) extern "C" fn nf_equal(a: Slot, b: Slot, ty: &TypeDesc) -> bool {
    value::equal(a, b, ty)
}

/// Replaces the value in `place` with `value`, releasing the old one.
///
/// # Safety
/// `place` is a valid slot holding a value of type `ty`, or zero.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn nf_store(place: *mut Slot, value: Slot, ty: &TypeDesc) {
    // SAFETY: by the caller's contract.
    let place = unsafe { &mut *place };
    value::release(std::mem::replace(place, value), ty);
}
