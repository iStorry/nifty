use lsp_types::{Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity};
use nifty_syntax::FileId;

use crate::document::Encoding;
use crate::project::Project;

const SOURCE: &str = "nifty";

/// The errors in `file`, with the other places each involves, which may be in other files.
pub(crate) fn diagnostics(project: &Project, file: FileId, encoding: Encoding) -> Vec<Diagnostic> {
    let document = project.document(file);
    project
        .diagnostics_in(file)
        .map(|diagnostic| {
            let related: Vec<_> = diagnostic
                .notes
                .iter()
                .map(|note| DiagnosticRelatedInformation {
                    location: project.location(note.span, encoding),
                    message: note.message.clone(),
                })
                .collect();
            Diagnostic {
                range: document.range(diagnostic.span, encoding),
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some(SOURCE.to_owned()),
                message: diagnostic.message.clone(),
                related_information: (!related.is_empty()).then_some(related),
                ..Diagnostic::default()
            }
        })
        .collect()
}
