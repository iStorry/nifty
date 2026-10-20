//! Every name in a project's files, each tied to the declaration it refers to when one
//! exists in the project. Built-in elements, built-in types, fields reached through a value and implicit
//! members have no declaration until the type checker exists, so they stay unresolved.

mod build;

use nifty_syntax::ast::File;
use nifty_syntax::{FileId, Span};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SymbolId(usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SymbolKind {
    State,
    Struct,
    Field,
    Function,
    View,
    Window,
    Parameter,
    Local,
}

/// Where in the project a symbol's name can be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Visibility {
    /// Top-level declarations, which may be used before they are declared and in every file.
    Everywhere,
    /// Parameters and `for` items, inside their body.
    Within(Span),
    /// Locals, from the end of their `let` to the end of the enclosing block or file.
    After {
        file: FileId,
        start: usize,
        end: Option<usize>,
    },
}

impl Visibility {
    fn contains(self, file: FileId, offset: usize) -> bool {
        match self {
            Self::Everywhere => true,
            Self::Within(span) => span.file == file && span.start < offset && offset < span.end,
            Self::After {
                file: declared_in,
                start,
                end,
            } => declared_in == file && start <= offset && end.is_none_or(|end| offset < end),
        }
    }
}

#[derive(Debug)]
pub(crate) struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    /// The struct a field belongs to, or the function or view a parameter belongs to.
    pub parent: Option<SymbolId>,
    pub visibility: Visibility,
}

/// What a name without a declaration in the project is used as.
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
    /// Sorted by file, then position. Names never overlap, so within a file this is also
    /// sorted by end.
    occurrences: Vec<Occurrence>,
}

impl Index {
    /// Indexes `files` as one program, which share their top-level names.
    pub(crate) fn new(files: &[&File]) -> Self {
        build::build(files)
    }

    pub(crate) fn symbol(&self, id: SymbolId) -> &Symbol {
        &self.symbols[id.0]
    }

    /// The names in `file`, in source order.
    pub(crate) fn occurrences_in(&self, file: FileId) -> &[Occurrence] {
        let start = self
            .occurrences
            .partition_point(|occurrence| occurrence.span.file < file);
        let end = self
            .occurrences
            .partition_point(|occurrence| occurrence.span.file <= file);
        &self.occurrences[start..end]
    }

    /// The name covering `offset` in `file`, including a cursor sitting just past its last
    /// character.
    pub(crate) fn occurrence_at(&self, file: FileId, offset: usize) -> Option<Occurrence> {
        let in_file = self.occurrences_in(file);
        let first_ending_at_or_after =
            in_file.partition_point(|occurrence| occurrence.span.end < offset);
        in_file
            .get(first_ending_at_or_after)
            .filter(|occurrence| occurrence.span.start <= offset)
            .copied()
    }

    /// The declaration and every reference of `id`, file by file in source order.
    pub(crate) fn occurrences_of(&self, id: SymbolId) -> impl Iterator<Item = Occurrence> {
        self.occurrences
            .iter()
            .filter(move |occurrence| occurrence.role.symbol() == Some(id))
            .copied()
    }

    /// The symbols whose names can be used at `offset` in `file`. Fields are only reached
    /// through a value or an argument label, so they are never in scope by themselves.
    pub(crate) fn in_scope(&self, file: FileId, offset: usize) -> impl Iterator<Item = &Symbol> {
        self.symbols.iter().filter(move |symbol| {
            symbol.kind != SymbolKind::Field && symbol.visibility.contains(file, offset)
        })
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
