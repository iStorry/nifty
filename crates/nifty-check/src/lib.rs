//! Semantic checking for Nifty: resolves names, checks types and element arguments, and
//! records which state every dynamic part of the UI reads.

mod checker;
pub mod elements;
pub mod hir;

use nifty_syntax::ast::File;
use nifty_syntax::{Diagnostic, Parsed};

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

/// Checks a parsed file and returns its program.
///
/// # Errors
///
/// Returns the syntax errors if the file didn't parse, and otherwise the type errors.
/// Type checking only runs on a file that parses, so one syntax error doesn't cause a
/// cascade of confusing type errors.
pub fn compile(parsed: &Parsed) -> Result<hir::Program, Vec<Diagnostic>> {
    if !parsed.diagnostics.is_empty() {
        return Err(parsed.diagnostics.clone());
    }
    let checked = check(&parsed.file);
    if checked.diagnostics.is_empty() {
        Ok(checked.program)
    } else {
        Err(checked.diagnostics)
    }
}

#[cfg(test)]
mod tests;
