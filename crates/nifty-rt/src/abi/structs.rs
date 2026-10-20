//! Structs: building, reading fields, and changing them in place.

use std::rc::Rc;
use std::slice;

use crate::value::{self, Slot, StructObject, TypeDesc};

/// A struct of type `ty` whose fields are taken, in order, from `fields`.
///
/// # Safety
/// `ty` is a struct type and `fields` points to one slot per field, of the field's type.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn nf_struct_new(ty: &'static TypeDesc, fields: *const Slot) -> Slot {
    let len = ty.children().len();
    let fields = if len == 0 {
        Box::default()
    } else {
        // SAFETY: by the caller's contract.
        unsafe { slice::from_raw_parts(fields, len) }.into()
    };
    value::into_slot(Rc::new(StructObject { ty, fields }))
}

/// The field at `index` of the borrowed struct, borrowed from it.
#[unsafe(no_mangle)]
pub(crate) extern "C" fn nf_struct_get(object: Slot, index: usize) -> Slot {
    value::as_struct(object).fields[index]
}

/// The slot of the field at `index` in the struct in `place`, which becomes its only
/// owner's so the field can be changed. The slot is valid until the struct next changes.
///
/// # Safety
/// `place` holds a struct with a field at `index`.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn nf_struct_field_mut(place: *mut Slot, index: usize) -> *mut Slot {
    // SAFETY: by the caller's contract.
    &raw mut value::struct_mut(unsafe { &mut *place }).fields[index]
}
