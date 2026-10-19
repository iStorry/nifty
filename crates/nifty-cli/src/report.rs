//! Renders diagnostics as annotated source snippets on stderr.

use std::io::{self, IsTerminal};

use ariadne::{Config, IndexType, Label, Report, ReportKind, Source};
use nifty_interp::RuntimeError;
use nifty_syntax::Diagnostic;

use crate::error::CliError;
use crate::frontend::SourceFile;

pub(crate) fn print(source: &SourceFile, diagnostics: &[Diagnostic]) -> Result<(), CliError> {
    write(&source.name(), &source.text, diagnostics)
        .map_err(|error| CliError::new(format!("can't write diagnostics: {error}")))
}

/// Reports an error the app hit while running, pointing at the code responsible.
pub(crate) fn runtime_error(source: &SourceFile, error: &RuntimeError) {
    let diagnostic = Diagnostic::new(&error.message, error.span);
    if let Err(write_error) = print(source, &[diagnostic]) {
        eprintln!("error: {}", error.message);
        eprintln!("(and the full report couldn't be written: {write_error})");
    }
}

fn write(file_name: &str, source: &str, diagnostics: &[Diagnostic]) -> io::Result<()> {
    // Nifty spans are byte offsets; ariadne counts characters unless told otherwise.
    let config = Config::default()
        .with_index_type(IndexType::Byte)
        .with_color(io::stderr().is_terminal());
    for diagnostic in diagnostics {
        let span = (file_name, diagnostic.span.range());
        Report::build(ReportKind::Error, span.clone())
            .with_config(config)
            .with_message(&diagnostic.message)
            .with_label(Label::new(span))
            .finish()
            .eprint((file_name, Source::from(source)))?;
    }
    let count = diagnostics.len();
    eprintln!("{count} error{}", if count == 1 { "" } else { "s" });
    Ok(())
}
