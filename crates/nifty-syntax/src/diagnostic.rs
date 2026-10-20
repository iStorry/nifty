use crate::span::Span;

/// An error in the source, pointing at the text that caused it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub message: String,
    pub span: Span,
    /// Other places the error involves, such as the first of two declarations.
    pub notes: Vec<Note>,
}

/// A secondary place a [`Diagnostic`] points at, with what it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    pub message: String,
    pub span: Span,
}

impl Diagnostic {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            notes: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_note(mut self, message: impl Into<String>, span: Span) -> Self {
        self.notes.push(Note {
            message: message.into(),
            span,
        });
        self
    }
}
