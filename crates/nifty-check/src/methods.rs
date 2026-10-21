//! The built-in methods on lists and strings. The checker dispatches on [`Method::kind`],
//! and tools such as the language server read the same table.

use nifty_runtime::TextMethod;

use crate::hir::Type;

/// A built-in method such as `items.push(item)`.
#[derive(Debug)]
pub struct Method {
    pub name: &'static str,
    pub kind: MethodKind,
    /// What it may be called on.
    pub receiver: Receiver,
    /// Its arguments, which are always given by position.
    pub params: &'static [MethodParam],
    pub summary: &'static str,
}

/// Which method a table entry is, for the checker to dispatch on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MethodKind {
    Len,
    IsEmpty,
    Push,
    Remove,
    /// `starts_with` and the other methods only strings have.
    Text(TextMethod),
}

/// The values a method may be called on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Receiver {
    Lists,
    Strings,
    ListsAndStrings,
}

impl Receiver {
    /// Whether a value of type `ty` has methods of this receiver.
    #[must_use]
    pub fn accepts(self, ty: &Type) -> bool {
        match ty {
            Type::List(_) => matches!(self, Self::Lists | Self::ListsAndStrings),
            Type::String => matches!(self, Self::Strings | Self::ListsAndStrings),
            _ => false,
        }
    }
}

#[derive(Debug)]
pub struct MethodParam {
    pub name: &'static str,
    pub kind: MethodParamKind,
}

/// What a method's argument must be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MethodParamKind {
    /// An item of the list the method is called on.
    Item,
    Int,
    String,
}

const fn param(name: &'static str, kind: MethodParamKind) -> MethodParam {
    MethodParam { name, kind }
}

const TEXT_PARAM: MethodParam = param("text", MethodParamKind::String);

const fn text_method(
    method: TextMethod,
    params: &'static [MethodParam],
    summary: &'static str,
) -> Method {
    Method {
        name: method.name(),
        kind: MethodKind::Text(method),
        receiver: Receiver::Strings,
        params,
        summary,
    }
}

pub const METHODS: &[Method] = &[
    Method {
        name: "len",
        kind: MethodKind::Len,
        receiver: Receiver::ListsAndStrings,
        params: &[],
        summary: "How many items a list has, or characters a string has.",
    },
    Method {
        name: "is_empty",
        kind: MethodKind::IsEmpty,
        receiver: Receiver::ListsAndStrings,
        params: &[],
        summary: "Whether a list or string has nothing in it.",
    },
    Method {
        name: "push",
        kind: MethodKind::Push,
        receiver: Receiver::Lists,
        params: &[param("item", MethodParamKind::Item)],
        summary: "Adds an item to the end of a list.",
    },
    Method {
        name: "remove",
        kind: MethodKind::Remove,
        receiver: Receiver::Lists,
        params: &[param("index", MethodParamKind::Int)],
        summary: "Removes the item at a position in a list.",
    },
    text_method(
        TextMethod::Contains,
        &[TEXT_PARAM],
        "Whether a string holds some text.",
    ),
    text_method(
        TextMethod::StartsWith,
        &[TEXT_PARAM],
        "Whether a string begins with some text.",
    ),
    text_method(
        TextMethod::EndsWith,
        &[TEXT_PARAM],
        "Whether a string ends with some text.",
    ),
    text_method(
        TextMethod::Find,
        &[TEXT_PARAM],
        "The position of the first character where some text first appears in a string, or `-1`.",
    ),
    text_method(
        TextMethod::Slice,
        &[
            param("start", MethodParamKind::Int),
            param("end", MethodParamKind::Int),
        ],
        "The characters of a string from `start` up to, but not including, `end`.",
    ),
    text_method(
        TextMethod::Split,
        &[param("separator", MethodParamKind::String)],
        "The pieces of a string between each separator, as a list.",
    ),
    text_method(
        TextMethod::Replace,
        &[
            param("from", MethodParamKind::String),
            param("to", MethodParamKind::String),
        ],
        "A string with each `from` replaced by `to`.",
    ),
    text_method(
        TextMethod::Trim,
        &[],
        "A string without the spaces and line breaks at its start and end.",
    ),
];

/// The method called `name`, if it is built in.
#[must_use]
pub fn method(name: &str) -> Option<&'static Method> {
    METHODS.iter().find(|method| method.name == name)
}

/// The names of the methods a value of type `ty` has, in table order.
pub fn methods_of(ty: &Type) -> impl Iterator<Item = &'static str> + '_ {
    METHODS
        .iter()
        .filter(|method| method.receiver.accepts(ty))
        .map(|method| method.name)
}
