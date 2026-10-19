//! Front end of the Nifty language: source text to a syntax tree, with diagnostics.

pub mod ast;
mod diagnostic;
mod lexer;
mod parser;
mod span;
mod token;

pub use diagnostic::Diagnostic;
pub use span::Span;

/// The result of parsing one source file. The tree is complete even when there are
/// diagnostics: parts that failed to parse are skipped or marked `ExprKind::Error`.
#[derive(Debug)]
pub struct Parsed {
    pub file: ast::File,
    pub diagnostics: Vec<Diagnostic>,
}

#[must_use]
pub fn parse(source: &str) -> Parsed {
    let (file, diagnostics) = parser::parse(source);
    Parsed { file, diagnostics }
}
