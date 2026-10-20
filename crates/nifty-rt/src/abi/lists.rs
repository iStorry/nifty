//! Lists: building, reading, and changing them in place.

use std::slice;

use nifty_runtime::Failure;

use super::{borrow_context, count};
use crate::context::Context;
use crate::value::{self, ListObject, Slot, TypeDesc};

/// A list of type `ty` holding `len` items from `items`, which it takes ownership of.
///
/// # Safety
/// `ty` is a list type and `items` points to `len` slots of its element type.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn nf_list_new(
    ty: &'static TypeDesc,
    items: *const Slot,
    len: usize,
) -> Slot {
    let items = if len == 0 {
        Vec::new()
    } else {
        // SAFETY: by the caller's contract.
        unsafe { slice::from_raw_parts(items, len) }.to_vec()
    };
    value::into_slot(std::rc::Rc::new(ListObject {
        element: ty.children()[0],
        items,
    }))
}

/// How many items the borrowed list holds.
#[unsafe(no_mangle)]
pub(crate) extern "C" fn nf_list_len(list: Slot) -> i64 {
    count(value::as_list(list).items.len())
}

/// Writes the item at `index` of the borrowed list to `out`, borrowed from the list.
///
/// # Safety
/// See the module's contract.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn nf_list_get(
    context: *const Context,
    list: Slot,
    index: i64,
    span: u32,
    out: *mut Slot,
) -> bool {
    // SAFETY: by the caller's contract.
    let context = unsafe { borrow_context(context) };
    let items = &value::as_list(list).items;
    match position(context, index, items.len(), span) {
        Some(position) => {
            // SAFETY: by the caller's contract.
            unsafe { out.write(items[position]) };
            true
        }
        None => false,
    }
}

/// Appends `item`, which it takes ownership of, to the list in `place`.
///
/// # Safety
/// `place` holds a list whose element type `item` has.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn nf_list_push(place: *mut Slot, item: Slot) {
    // SAFETY: by the caller's contract.
    value::list_mut(unsafe { &mut *place }).items.push(item);
}

/// Removes the item at `index` from the list in `place`.
///
/// # Safety
/// See the module's contract.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn nf_list_remove(
    context: *const Context,
    place: *mut Slot,
    index: i64,
    span: u32,
) -> bool {
    // SAFETY: by the caller's contract.
    let (context, place) = unsafe { (borrow_context(context), &mut *place) };
    let len = value::as_list(*place).items.len();
    let Some(position) = position(context, index, len, span) else {
        return false;
    };
    let list = value::list_mut(place);
    value::release(list.items.remove(position), list.element);
    true
}

/// Writes to `out` the slot of the item at `index` in the list in `place`, which becomes
/// its only owner's so the item can be changed. The slot is valid until the list next
/// changes.
///
/// # Safety
/// See the module's contract.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn nf_list_item_mut(
    context: *const Context,
    place: *mut Slot,
    index: i64,
    span: u32,
    out: *mut *mut Slot,
) -> bool {
    // SAFETY: by the caller's contract.
    let (context, place) = unsafe { (borrow_context(context), &mut *place) };
    let len = value::as_list(*place).items.len();
    let Some(position) = position(context, index, len, span) else {
        return false;
    };
    let item = &raw mut value::list_mut(place).items[position];
    // SAFETY: by the caller's contract.
    unsafe { out.write(item) };
    true
}

/// The item position `index` names in a list of `len`, or `None` after recording why
/// there's no such item.
fn position(context: &Context, index: i64, len: usize, span: u32) -> Option<usize> {
    let Ok(position) = usize::try_from(index) else {
        context.fail(Failure::NegativePosition(index), span);
        return None;
    };
    if position >= len {
        context.fail(Failure::NoItem { position, len }, span);
        return None;
    }
    Some(position)
}
