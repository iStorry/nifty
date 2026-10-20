//! Semantic checking for Nifty: resolves names, checks types and element arguments, and
//! records which state every dynamic part of the UI reads.

mod checker;
pub mod elements;
pub mod functions;
pub mod hir;
pub mod methods;

use nifty_syntax::ast::File;
use nifty_syntax::{Diagnostic, Parsed};

/// The checked program. Run it only when `diagnostics` is empty: parts with errors are
/// left out or marked with `Type::Error`.
#[derive(Debug)]
pub struct Checked {
    pub program: hir::Program,
    pub diagnostics: Vec<Diagnostic>,
}

/// Checks `files` as one program. They share one namespace, so a name declared in any of
/// them can be used in all of them. The first is the entry file.
#[must_use]
pub fn check(files: &[&File]) -> Checked {
    let (program, diagnostics) = checker::check(files);
    Checked {
        program,
        diagnostics,
    }
}

/// Checks a program's parsed files and returns the program; the first is the entry file.
///
/// # Errors
///
/// Returns the syntax errors if any file didn't parse, and otherwise the type errors.
/// Type checking only runs on files that parse, so one syntax error doesn't cause a
/// cascade of confusing type errors.
pub fn compile(parsed: &[&Parsed]) -> Result<hir::Program, Vec<Diagnostic>> {
    let syntax_errors: Vec<Diagnostic> = parsed
        .iter()
        .flat_map(|file| file.diagnostics.iter().cloned())
        .collect();
    if !syntax_errors.is_empty() {
        return Err(syntax_errors);
    }
    let files: Vec<&File> = parsed.iter().map(|file| &file.file).collect();
    let checked = check(&files);
    if checked.diagnostics.is_empty() {
        Ok(checked.program)
    } else {
        Err(checked.diagnostics)
    }
}

#[cfg(test)]
mod tests;
