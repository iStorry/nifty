//! The built-in methods on lists and strings. The checker dispatches on [`Method::kind`],
//! and tools such as the language server read the same table.

/// A built-in method such as `items.push(item)`.
#[derive(Debug)]
pub struct Method {
    pub name: &'static str,
    pub kind: MethodKind,
    /// The names of its arguments, which are always given by position.
    pub params: &'static [&'static str],
    pub summary: &'static str,
}

/// Which method a table entry is, for the checker to dispatch on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MethodKind {
    Len,
    IsEmpty,
    Push,
    Remove,
}

pub const METHODS: &[Method] = &[
    Method {
        name: "len",
        kind: MethodKind::Len,
        params: &[],
        summary: "How many items a list has, or characters a string has.",
    },
    Method {
        name: "is_empty",
        kind: MethodKind::IsEmpty,
        params: &[],
        summary: "Whether a list or string has nothing in it.",
    },
    Method {
        name: "push",
        kind: MethodKind::Push,
        params: &["item"],
        summary: "Adds an item to the end of a list.",
    },
    Method {
        name: "remove",
        kind: MethodKind::Remove,
        params: &["index"],
        summary: "Removes the item at a position in a list.",
    },
];

/// The method called `name`, if it is built in.
#[must_use]
pub fn method(name: &str) -> Option<&'static Method> {
    METHODS.iter().find(|method| method.name == name)
}
