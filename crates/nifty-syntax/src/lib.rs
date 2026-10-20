//! Front end of the Nifty language: source text to a syntax tree, with diagnostics.

pub mod ast;
mod diagnostic;
mod lexer;
mod parser;
pub mod project;
mod span;
mod token;

pub use diagnostic::{Diagnostic, Note};
pub use lexer::ESCAPES;
pub use span::{FileId, Span};
pub use token::{KEYWORDS, SYMBOLS, Token, TokenKind};

/// The result of parsing one source file. The tree is complete even when there are
/// diagnostics: parts that failed to parse are skipped or marked `ExprKind::Error`.
#[derive(Debug)]
pub struct Parsed {
    pub file: ast::File,
    pub diagnostics: Vec<Diagnostic>,
}

/// The tokens of `source`, ending with `Eof`. Lexing errors are left to [`parse`].
#[must_use]
pub fn tokens(source: &str, file: FileId) -> Vec<Token> {
    lexer::lex(source, file).0
}

/// `text` as a Nifty string literal, quotes included, with every special character escaped.
#[must_use]
pub fn string_literal(text: &str) -> String {
    format!("\"{}\"", lexer::escape(text))
}

/// Parses the text of `file`; every span in the result points into it.
#[must_use]
pub fn parse(source: &str, file: FileId) -> Parsed {
    let (file, diagnostics) = parser::parse(source, file);
    Parsed { file, diagnostics }
}
