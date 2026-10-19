//! Every name in a file, each tied to the declaration it refers to when one exists in the
//! file. Built-in elements, built-in types, fields reached through a value and implicit
//! members have no declaration until the type checker exists, so they stay unresolved.

mod build;

use nifty_syntax::Span;
use nifty_syntax::ast::File;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SymbolId(usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SymbolKind {
    State,
    Struct,
    Field,
    Function,
    View,
    Parameter,
    Local,
}

#[derive(Debug)]
pub(crate) struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    /// The struct a field belongs to, or the function or view a parameter belongs to.
    pub parent: Option<SymbolId>,
}

/// What a name without a declaration in the file is used as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Unresolved {
    Function,
    Type,
    Variable,
    Field,
    Method,
    ImplicitMember,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Role {
    Declaration(SymbolId),
    Reference(SymbolId),
    Unresolved(Unresolved),
}

impl Role {
    pub(crate) fn symbol(self) -> Option<SymbolId> {
        match self {
            Self::Declaration(id) | Self::Reference(id) => Some(id),
            Self::Unresolved(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Occurrence {
    pub span: Span,
    pub role: Role,
}

#[derive(Debug)]
pub(crate) struct Index {
    symbols: Vec<Symbol>,
    /// Sorted by position. Names never overlap, so this is also sorted by end.
    occurrences: Vec<Occurrence>,
}

impl Index {
    pub(crate) fn new(file: &File) -> Self {
        build::build(file)
    }

    pub(crate) fn symbol(&self, id: SymbolId) -> &Symbol {
        &self.symbols[id.0]
    }

    pub(crate) fn occurrences(&self) -> &[Occurrence] {
        &self.occurrences
    }

    /// The name covering `offset`, including a cursor sitting just past its last character.
    pub(crate) fn occurrence_at(&self, offset: usize) -> Option<Occurrence> {
        let first_ending_at_or_after = self
            .occurrences
            .partition_point(|occurrence| occurrence.span.end < offset);
        self.occurrences
            .get(first_ending_at_or_after)
            .filter(|occurrence| occurrence.span.start <= offset)
            .copied()
    }

    /// The declaration and every reference of `id`, in source order.
    pub(crate) fn occurrences_of(&self, id: SymbolId) -> impl Iterator<Item = Occurrence> {
        self.occurrences
            .iter()
            .filter(move |occurrence| occurrence.role.symbol() == Some(id))
            .copied()
    }

    pub(crate) fn declaration_of(&self, id: SymbolId) -> Option<Span> {
        self.occurrences
            .iter()
            .find(|occurrence| occurrence.role == Role::Declaration(id))
            .map(|occurrence| occurrence.span)
    }
}

#[cfg(test)]
mod tests;
