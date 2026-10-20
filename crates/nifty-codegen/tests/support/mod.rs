//! What the codegen tests share: checking a source, and saying where its spans are.

use nifty_check::hir::Program;
use nifty_rt::SourceLocation;
use nifty_syntax::{FileId, Span};

/// The file name errors in test programs point at.
pub const FILE: &str = "main.nf";

pub fn check(source: &str) -> Program {
    let parsed = nifty_syntax::parse(source, FileId::default());
    assert_eq!(parsed.diagnostics, [], "the test source must parse");
    let checked = nifty_check::check(&[&parsed.file]);
    assert_eq!(checked.diagnostics, [], "the test source must check");
    checked.program
}

/// Where `span` starts in `source`, counting lines and columns from 1.
pub fn locate(source: &str, span: Span) -> SourceLocation {
    let before = &source[..span.start];
    let line = before.matches('\n').count() + 1;
    let column = before.len() - before.rfind('\n').map_or(0, |newline| newline + 1) + 1;
    SourceLocation {
        file: FILE.to_owned(),
        line: u32::try_from(line).expect("test sources are short"),
        column: u32::try_from(column).expect("test sources are short"),
    }
}
