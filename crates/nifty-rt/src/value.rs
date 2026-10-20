//! Values as compiled code holds them: every value fits in a 64-bit [`Slot`].
//!
//! An `int` is its two's-complement bits, a `float` its IEEE bits, a `bool` 0 or 1, and a
//! string, list or struct a pointer to a reference-counted object. Zero in a slot of one of
//! those types means "no value yet", and retaining or releasing it does nothing. Lists and
//! structs share their contents until a copy is changed, so values keep value semantics.

use std::cmp::Ordering;
use std::fmt::{self, Write as _};
use std::rc::Rc;

use crate::ffi::Slice;

pub type Slot = u64;

/// What kind of value a [`TypeDesc`] describes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum TypeKind {
    Int,
    Float,
    Bool,
    String,
    /// Its children are the element type.
    List,
    /// Its children are the field types, in order.
    Struct,
}

/// A type, as a compiled program describes it. Struct types may lead back to themselves
/// through their fields, so these form a graph of statics.
#[derive(Debug)]
#[repr(C)]
pub struct TypeDesc {
    kind: TypeKind,
    children: Slice<&'static TypeDesc>,
}

/// The built-in types, exported so compiled programs can point at them; [`crate::symbols`]
/// names them.
#[unsafe(export_name = "nf_type_int")]
pub static INT: TypeDesc = TypeDesc::scalar(TypeKind::Int);
#[unsafe(export_name = "nf_type_float")]
pub static FLOAT: TypeDesc = TypeDesc::scalar(TypeKind::Float);
#[unsafe(export_name = "nf_type_bool")]
pub static BOOL: TypeDesc = TypeDesc::scalar(TypeKind::Bool);
#[unsafe(export_name = "nf_type_string")]
pub static STRING: TypeDesc = TypeDesc::scalar(TypeKind::String);

impl TypeDesc {
    const fn scalar(kind: TypeKind) -> Self {
        Self {
            kind,
            children: Slice::EMPTY,
        }
    }

    #[must_use]
    pub const fn list(element: &'static [&'static Self; 1]) -> Self {
        Self {
            kind: TypeKind::List,
            children: Slice::new(element),
        }
    }

    #[must_use]
    pub const fn structure(fields: &'static [&'static Self]) -> Self {
        Self {
            kind: TypeKind::Struct,
            children: Slice::new(fields),
        }
    }

    #[must_use]
    pub fn kind(&self) -> TypeKind {
        self.kind
    }

    /// The element type of a list, or the field types of a struct.
    #[must_use]
    pub fn children(&self) -> &'static [&'static Self] {
        self.children.items()
    }

    fn element(&self) -> &'static Self {
        self.children()[0]
    }

    fn is_object(&self) -> bool {
        matches!(
            self.kind,
            TypeKind::String | TypeKind::List | TypeKind::Struct
        )
    }
}

pub(crate) struct StringObject(pub Box<str>);

pub(crate) struct ListObject {
    pub element: &'static TypeDesc,
    pub items: Vec<Slot>,
}

pub(crate) struct StructObject {
    pub ty: &'static TypeDesc,
    pub fields: Box<[Slot]>,
}

impl Clone for ListObject {
    fn clone(&self) -> Self {
        for item in &self.items {
            retain(*item, self.element);
        }
        Self {
            element: self.element,
            items: self.items.clone(),
        }
    }
}

impl Drop for ListObject {
    fn drop(&mut self) {
        for item in &self.items {
            release(*item, self.element);
        }
    }
}

impl Clone for StructObject {
    fn clone(&self) -> Self {
        for (field, ty) in self.fields.iter().zip(self.ty.children()) {
            retain(*field, ty);
        }
        Self {
            ty: self.ty,
            fields: self.fields.clone(),
        }
    }
}

impl Drop for StructObject {
    fn drop(&mut self) {
        for (field, ty) in self.fields.iter().zip(self.ty.children()) {
            release(*field, ty);
        }
    }
}

/// The slot holding an object, which the slot now owns.
pub(crate) fn into_slot<T>(object: Rc<T>) -> Slot {
    Rc::into_raw(object).expose_provenance() as Slot
}

fn pointer<T>(slot: Slot) -> *const T {
    let address = usize::try_from(slot).expect("Nifty only runs on 64-bit systems");
    std::ptr::with_exposed_provenance(address)
}

/// Takes the object a slot owns.
///
/// # Safety
/// `slot` came from [`into_slot`] with the same `T`, and its ownership passes to the result.
pub(crate) unsafe fn from_slot<T>(slot: Slot) -> Rc<T> {
    // SAFETY: by the caller's contract, the pointer came from `Rc::into_raw::<T>`.
    unsafe { Rc::from_raw(pointer(slot)) }
}

/// The object a slot holds, borrowed for as long as the slot keeps it alive.
///
/// # Safety
/// `slot` came from [`into_slot`] with the same `T`, and is still owned by someone for `'a`.
pub(crate) unsafe fn borrow<'a, T>(slot: Slot) -> &'a T {
    // SAFETY: by the caller's contract, the pointer leads to a live `T` inside an `Rc`.
    unsafe { &*pointer::<T>(slot) }
}

/// Adds an owner to the value in `slot`.
pub(crate) fn retain(slot: Slot, ty: &TypeDesc) {
    if slot == 0 || !ty.is_object() {
        return;
    }
    // SAFETY: a non-zero slot of an object type holds a pointer from `into_slot` with the
    // object type its kind names; compiled code keeps slots and types together.
    unsafe {
        match ty.kind() {
            TypeKind::String => Rc::increment_strong_count(pointer::<StringObject>(slot)),
            TypeKind::List => Rc::increment_strong_count(pointer::<ListObject>(slot)),
            TypeKind::Struct => Rc::increment_strong_count(pointer::<StructObject>(slot)),
            TypeKind::Int | TypeKind::Float | TypeKind::Bool => {}
        }
    }
}

/// Removes an owner from the value in `slot`, freeing it with its last owner.
pub(crate) fn release(slot: Slot, ty: &TypeDesc) {
    if slot == 0 || !ty.is_object() {
        return;
    }
    // SAFETY: as in `retain`; the caller gives up the owner this removes.
    unsafe {
        match ty.kind() {
            TypeKind::String => Rc::decrement_strong_count(pointer::<StringObject>(slot)),
            TypeKind::List => Rc::decrement_strong_count(pointer::<ListObject>(slot)),
            TypeKind::Struct => Rc::decrement_strong_count(pointer::<StructObject>(slot)),
            TypeKind::Int | TypeKind::Float | TypeKind::Bool => {}
        }
    }
}

pub(crate) fn string_slot(text: impl Into<Box<str>>) -> Slot {
    into_slot(Rc::new(StringObject(text.into())))
}

/// The text of a string slot that someone owns for `'a`.
pub(crate) fn as_str<'a>(slot: Slot) -> &'a str {
    // SAFETY: compiled code passes only string slots here, and they stay owned while used.
    unsafe { &borrow::<StringObject>(slot).0 }
}

pub(crate) fn as_list<'a>(slot: Slot) -> &'a ListObject {
    // SAFETY: compiled code passes only list slots here, and they stay owned while used.
    unsafe { borrow(slot) }
}

pub(crate) fn as_struct<'a>(slot: Slot) -> &'a StructObject {
    // SAFETY: compiled code passes only struct slots here, and they stay owned while used.
    unsafe { borrow(slot) }
}

/// Makes the list in `place` its only owner's, copying it if shared, and returns it.
pub(crate) fn list_mut(place: &mut Slot) -> &mut ListObject {
    unique::<ListObject>(place)
}

pub(crate) fn struct_mut(place: &mut Slot) -> &mut StructObject {
    unique::<StructObject>(place)
}

fn unique<T: Clone>(place: &mut Slot) -> &mut T {
    // SAFETY: `place` owns its object, whose type the caller names.
    let mut object = unsafe { from_slot::<T>(*place) };
    Rc::make_mut(&mut object);
    *place = into_slot(object);
    // SAFETY: the object is now uniquely owned by `place`, which the borrow keeps alive.
    unsafe { &mut *pointer::<T>(*place).cast_mut() }
}

pub(crate) fn int(slot: Slot) -> i64 {
    i64::from_ne_bytes(slot.to_ne_bytes())
}

pub(crate) fn int_slot(value: i64) -> Slot {
    Slot::from_ne_bytes(value.to_ne_bytes())
}

/// Whether two values of type `ty` are equal, as `==` decides.
pub(crate) fn equal(a: Slot, b: Slot, ty: &TypeDesc) -> bool {
    match ty.kind() {
        TypeKind::Int | TypeKind::Bool => a == b,
        // IEEE equality, as `==` on floats: NaN is unequal to everything, and -0 equals 0.
        TypeKind::Float => {
            f64::from_bits(a).partial_cmp(&f64::from_bits(b)) == Some(Ordering::Equal)
        }
        TypeKind::String => as_str(a) == as_str(b),
        TypeKind::List => {
            let (a, b) = (as_list(a), as_list(b));
            a.items.len() == b.items.len()
                && a.items
                    .iter()
                    .zip(&b.items)
                    .all(|(a, b)| equal(*a, *b, ty.element()))
        }
        TypeKind::Struct => {
            let (a, b) = (as_struct(a), as_struct(b));
            a.fields
                .iter()
                .zip(b.fields.iter())
                .zip(ty.children())
                .all(|((a, b), ty)| equal(*a, *b, ty))
        }
    }
}

/// Writes a value of type `ty` as it reads inside an interpolated string.
pub(crate) fn write(text: &mut String, slot: Slot, ty: &TypeDesc) {
    let written = match ty.kind() {
        TypeKind::Int => write!(text, "{}", int(slot)),
        TypeKind::Float => write!(text, "{}", f64::from_bits(slot)),
        TypeKind::Bool => write!(text, "{}", slot != 0),
        TypeKind::String => text.write_str(as_str(slot)),
        TypeKind::List => {
            let list = as_list(slot);
            write_items(text, list.items.iter().map(|item| (*item, list.element)));
            Ok(())
        }
        TypeKind::Struct => {
            let object = as_struct(slot);
            let fields = object.fields.iter().copied();
            write_items(text, fields.zip(object.ty.children().iter().copied()));
            Ok(())
        }
    };
    written.expect("writing to a String cannot fail");
}

fn write_items(text: &mut String, items: impl Iterator<Item = (Slot, &'static TypeDesc)>) {
    text.push('[');
    for (index, (item, ty)) in items.enumerate() {
        if index > 0 {
            text.push_str(", ");
        }
        write(text, item, ty);
    }
    text.push(']');
}

/// A value the runtime owns, released when dropped.
pub(crate) struct Value {
    slot: Slot,
    ty: &'static TypeDesc,
}

impl Value {
    /// Takes ownership of `slot`.
    pub(crate) fn owned(slot: Slot, ty: &'static TypeDesc) -> Self {
        Self { slot, ty }
    }

    pub(crate) fn slot(&self) -> Slot {
        self.slot
    }
}

impl Clone for Value {
    fn clone(&self) -> Self {
        retain(self.slot, self.ty);
        Self {
            slot: self.slot,
            ty: self.ty,
        }
    }
}

impl Drop for Value {
    fn drop(&mut self) {
        release(self.slot, self.ty);
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut text = String::new();
        write(&mut text, self.slot, self.ty);
        f.debug_tuple("Value").field(&text).finish()
    }
}

#[cfg(test)]
mod tests;
