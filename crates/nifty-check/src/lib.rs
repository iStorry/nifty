//! Semantic checking for Nifty: resolves names, checks types and element arguments, and
//! records which state every dynamic part of the UI reads.

mod checker;
pub mod hir;

use nifty_syntax::Diagnostic;
use nifty_syntax::ast::File;

/// The checked program. Run it only when `diagnostics` is empty: parts with errors are
/// left out or marked with `Type::Error`.
#[derive(Debug)]
pub struct Checked {
    pub program: hir::Program,
    pub diagnostics: Vec<Diagnostic>,
}

#[must_use]
pub fn check(file: &File) -> Checked {
    let (program, diagnostics) = checker::check(file);
    Checked {
        program,
        diagnostics,
    }
}

#[cfg(test)]
mod tests;
