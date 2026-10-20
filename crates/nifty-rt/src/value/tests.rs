use std::mem::ManuallyDrop;

use super::*;
use crate::abi::{
    nf_list_get, nf_list_item_mut, nf_list_len, nf_list_new, nf_list_push, nf_list_remove,
    nf_string_compare, nf_string_concat, nf_string_len, nf_struct_field_mut, nf_struct_get,
    nf_struct_new,
};

static LIST_OF_STRINGS: TypeDesc = TypeDesc::list(&[&STRING]);
static LIST_OF_INTS: TypeDesc = TypeDesc::list(&[&INT]);
/// `struct Point { x: int, tags: [string] }`
static POINT: TypeDesc = TypeDesc::structure(&[&INT, &LIST_OF_STRINGS]);
/// `struct Tree { label: string, children: [Tree] }`, which leads back to itself.
static TREE: TypeDesc = TypeDesc::structure(&[&STRING, &TREES]);
static TREES: TypeDesc = TypeDesc::list(&[&TREE]);

fn string(text: &str) -> Value {
    Value::owned(string_slot(text), &STRING)
}

fn list(ty: &'static TypeDesc, items: Vec<Value>) -> Value {
    let slots: Vec<Slot> = items
        .into_iter()
        .map(|item| ManuallyDrop::new(item).slot())
        .collect();
    // SAFETY: the slots are owned values of the list's element type, given to the list.
    Value::owned(unsafe { nf_list_new(ty, slots.as_ptr(), slots.len()) }, ty)
}

fn shown(value: &Value) -> String {
    let mut text = String::new();
    write(&mut text, value.slot(), value.ty);
    text
}

/// How many owners the object in `slot` has.
fn owners<T>(slot: Slot) -> usize {
    // SAFETY: the slot holds a `T` that the test keeps alive; it isn't dropped here.
    let object = ManuallyDrop::new(unsafe { from_slot::<T>(slot) });
    Rc::strong_count(&object)
}

#[test]
fn strings_concatenate_count_characters_and_compare() {
    let (a, b) = (string("héllo "), string("wörld"));
    let joined = Value::owned(nf_string_concat(a.slot(), b.slot()), &STRING);
    assert_eq!(as_str(joined.slot()), "héllo wörld");
    assert_eq!(nf_string_len(joined.slot()), 11);
    assert_eq!(nf_string_compare(a.slot(), b.slot()), -1);
    assert_eq!(nf_string_compare(b.slot(), b.slot()), 0);
}

#[test]
fn values_read_as_they_do_in_the_interpreter() {
    let tags = list(&LIST_OF_STRINGS, vec![string("a"), string("b")]);
    let fields = [int_slot(-3), ManuallyDrop::new(tags).slot()];
    // SAFETY: the fields are owned values of the struct's field types.
    let point = Value::owned(unsafe { nf_struct_new(&POINT, fields.as_ptr()) }, &POINT);
    assert_eq!(shown(&point), "[-3, [a, b]]");
    assert_eq!(shown(&Value::owned(1.5_f64.to_bits(), &FLOAT)), "1.5");
    assert_eq!(shown(&Value::owned(1, &BOOL)), "true");
}

#[test]
fn copies_share_until_one_is_changed() {
    let original = list(&LIST_OF_STRINGS, vec![string("a")]);
    let mut copy = original.clone();
    assert_eq!(owners::<ListObject>(original.slot()), 2);

    // SAFETY: `copy` owns a list of strings, and the new item is an owned string.
    unsafe { nf_list_push(&raw mut copy.slot, string_slot("b")) };

    assert_eq!(shown(&original), "[a]");
    assert_eq!(shown(&copy), "[a, b]");
    assert_eq!(owners::<ListObject>(original.slot()), 1);
    let first = as_list(original.slot()).items[0];
    assert_eq!(
        owners::<StringObject>(first),
        2,
        "the copy shares the unchanged item"
    );
}

#[test]
fn changing_a_field_of_a_shared_struct_copies_only_the_struct() {
    let tags = list(&LIST_OF_STRINGS, vec![string("x")]);
    let fields = [int_slot(1), ManuallyDrop::new(tags.clone()).slot()];
    // SAFETY: the fields are owned values of the struct's field types.
    let original = Value::owned(unsafe { nf_struct_new(&POINT, fields.as_ptr()) }, &POINT);
    let mut copy = original.clone();

    // SAFETY: `copy` owns a `Point`, whose field 0 is an int.
    unsafe { *nf_struct_field_mut(&raw mut copy.slot, 0) = int_slot(2) };

    assert_eq!(shown(&original), "[1, [x]]");
    assert_eq!(shown(&copy), "[2, [x]]");
    assert_eq!(
        nf_struct_get(copy.slot(), 1),
        tags.slot(),
        "the tags stay shared"
    );
}

#[test]
fn equality_compares_contents() {
    let a = list(&LIST_OF_STRINGS, vec![string("a"), string("b")]);
    let b = list(&LIST_OF_STRINGS, vec![string("a"), string("b")]);
    let c = list(&LIST_OF_STRINGS, vec![string("a")]);
    assert!(equal(a.slot(), b.slot(), &LIST_OF_STRINGS));
    assert!(!equal(a.slot(), c.slot(), &LIST_OF_STRINGS));
    let nan = f64::NAN.to_bits();
    assert!(
        !equal(nan, nan, &FLOAT),
        "NaN is unequal to itself, as in the interpreter"
    );
}

#[test]
fn types_that_lead_back_to_themselves_are_released_completely() {
    let leaf = |label: &str| {
        let fields = [
            string_slot(label),
            ManuallyDrop::new(list(&TREES, Vec::new())).slot(),
        ];
        // SAFETY: the fields are owned values of the struct's field types.
        Value::owned(unsafe { nf_struct_new(&TREE, fields.as_ptr()) }, &TREE)
    };
    let child = leaf("child");
    let kept = child.clone();
    let fields = [
        string_slot("root"),
        ManuallyDrop::new(list(&TREES, vec![child])).slot(),
    ];
    // SAFETY: the fields are owned values of the struct's field types.
    let root = Value::owned(unsafe { nf_struct_new(&TREE, fields.as_ptr()) }, &TREE);
    assert_eq!(shown(&root), "[root, [[child, []]]]");
    assert_eq!(owners::<StructObject>(kept.slot()), 2);

    drop(root);

    assert_eq!(owners::<StructObject>(kept.slot()), 1);
}

#[test]
fn list_positions_are_checked() {
    let context = crate::tests::empty_context();
    let numbers = list(&LIST_OF_INTS, vec![Value::owned(int_slot(7), &INT)]);
    let mut out = 0;
    // SAFETY: the context is live, `numbers` is a list, and `out` is writable.
    let ok = unsafe { nf_list_get(&raw const context, numbers.slot(), 0, 0, &raw mut out) };
    assert!(ok);
    assert_eq!(int(out), 7);
    assert_eq!(nf_list_len(numbers.slot()), 1);

    // SAFETY: as above.
    let missing = unsafe { nf_list_get(&raw const context, numbers.slot(), 1, 0, &raw mut out) };
    assert!(!missing);
    assert_eq!(
        context.take_error().message,
        "there is no item 1; the list has 1"
    );

    let mut numbers = numbers;
    let mut item = std::ptr::null_mut();
    // SAFETY: as above, with `numbers` owned by this test.
    let negative = unsafe {
        nf_list_item_mut(
            &raw const context,
            &raw mut numbers.slot,
            -1,
            0,
            &raw mut item,
        )
    };
    assert!(!negative);
    assert_eq!(
        context.take_error().message,
        "list positions start at 0, so -1 isn't one"
    );
    // SAFETY: as above.
    assert!(unsafe { nf_list_remove(&raw const context, &raw mut numbers.slot, 0, 0) });
    assert_eq!(nf_list_len(numbers.slot()), 0);
}
