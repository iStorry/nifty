//! Renders diagnostics as annotated source snippets on stderr.

use std::io::{self, IsTerminal};

use ariadne::{Config, IndexType, Label, Report, ReportKind, Source};
use nifty_syntax::Diagnostic;

pub(crate) fn print(file_name: &str, source: &str, diagnostics: &[Diagnostic]) -> io::Result<()> {
    // nifty spans are byte offsets; ariadne counts characters unless told otherwise.
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
