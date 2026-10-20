//! Paths into a value, shared by the interpreter's and compiled apps' places.

/// One step from a value into part of it: a struct's field, or a list's item, by position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Field(usize),
    Index(usize),
}
