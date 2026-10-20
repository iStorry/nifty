//! The built-in functions and the arguments each one takes. The checker dispatches on
//! [`Function::kind`], and tools such as the language server read the same table.

use crate::elements::{BoundType, Param, ParamKind, positional, required};

/// A built-in function such as `download`, called from functions and button actions.
#[derive(Debug)]
pub struct Function {
    pub name: &'static str,
    pub kind: FunctionKind,
    pub params: &'static [Param],
    pub summary: &'static str,
}

/// Which function a table entry is, for the checker to dispatch on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FunctionKind {
    Download,
}

/// Saves a file from the web in the background, reporting into two bound states.
pub const DOWNLOAD: &str = "download";

pub const FUNCTIONS: &[Function] = &[Function {
    name: DOWNLOAD,
    kind: FunctionKind::Download,
    params: &[
        positional("url", ParamKind::Text),
        required("to", ParamKind::Text),
        required("progress", ParamKind::Binding(BoundType::Int)),
        required("status", ParamKind::Binding(BoundType::String)),
    ],
    summary: "Saves the file at a web address into a folder, in the background.",
}];

/// The function called `name`, if it is built in.
#[must_use]
pub fn function(name: &str) -> Option<&'static Function> {
    FUNCTIONS.iter().find(|function| function.name == name)
}
