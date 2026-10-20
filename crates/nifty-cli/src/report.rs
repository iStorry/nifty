//! Renders diagnostics as annotated source snippets on stderr.

use std::io::{self, IsTerminal};

use ariadne::{Config, IndexType, Label, Report, ReportKind};
use nifty_interp::RuntimeError;
use nifty_syntax::{Diagnostic, Span};

use crate::error::CliError;
use crate::frontend::Sources;

pub(crate) fn print(sources: &Sources, diagnostics: &[Diagnostic]) -> Result<(), CliError> {
    write(sources, diagnostics)
        .map_err(|error| CliError::new(format!("can't write diagnostics: {error}")))
}

/// Reports an error the app hit while running, pointing at the code responsible.
pub(crate) fn runtime_error(sources: &Sources, error: &RuntimeError) {
    let diagnostic = Diagnostic::new(&error.message, error.span);
    if let Err(write_error) = print(sources, &[diagnostic]) {
        eprintln!("error: {}", error.message);
        eprintln!("(and the full report couldn't be written: {write_error})");
    }
}

fn write(sources: &Sources, diagnostics: &[Diagnostic]) -> io::Result<()> {
    // Nifty spans are byte offsets; ariadne counts characters unless told otherwise.
    let config = Config::default()
        .with_index_type(IndexType::Byte)
        .with_color(io::stderr().is_terminal());
    let mut cache = ariadne::sources(
        sources
            .files()
            .iter()
            .map(|file| (file.name(), file.text.as_str())),
    );
    let place = |span: Span| (sources.file(span.file).name(), span.range());
    for diagnostic in diagnostics {
        let notes = diagnostic
            .notes
            .iter()
            .map(|note| Label::new(place(note.span)).with_message(&note.message));
        Report::build(ReportKind::Error, place(diagnostic.span))
            .with_config(config)
            .with_message(&diagnostic.message)
            .with_label(Label::new(place(diagnostic.span)))
            .with_labels(notes)
            .finish()
            .eprint(&mut cache)?;
    }
    let count = diagnostics.len();
    eprintln!("{count} error{}", if count == 1 { "" } else { "s" });
    Ok(())
}
