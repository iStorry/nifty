use lsp_types::{Diagnostic, DiagnosticSeverity};

use crate::document::{Document, Encoding};

const SOURCE: &str = "nifty";

pub(crate) fn diagnostics(document: &Document, encoding: Encoding) -> Vec<Diagnostic> {
    document
        .diagnostics
        .iter()
        .map(|diagnostic| Diagnostic {
            range: document.range(diagnostic.span, encoding),
            severity: Some(DiagnosticSeverity::ERROR),
            source: Some(SOURCE.to_owned()),
            message: diagnostic.message.clone(),
            ..Diagnostic::default()
        })
        .collect()
}
