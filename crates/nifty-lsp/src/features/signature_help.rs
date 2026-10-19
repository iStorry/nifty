//! The parameters of the call being written, with the one being filled in highlighted.

use lsp_types::{
    Documentation, ParameterInformation, ParameterLabel, SignatureHelp, SignatureInformation,
};

use crate::context;
use crate::document::{Document, to_u32};
use crate::signature;

pub(crate) fn signature_help(document: &Document, offset: usize) -> Option<SignatureHelp> {
    let word_start = document.word_start(offset);
    let call = context::at(document.source(), word_start).call?;
    let signature = signature::of(document, call.callee.as_deref()?)?;
    let active = signature.active(call.label.as_deref(), call.positional);
    let (label, offsets) = signature.label();
    let parameters = signature
        .params
        .iter()
        .zip(offsets)
        .map(|(param, offset)| ParameterInformation {
            label: ParameterLabel::LabelOffsets(offset),
            documentation: (!param.positional)
                .then(|| Documentation::String(format!("Named: `{}: …`", param.name))),
        })
        .collect();
    Some(SignatureHelp {
        signatures: vec![SignatureInformation {
            label,
            documentation: signature
                .summary
                .map(|summary| Documentation::String(summary.to_owned())),
            parameters: Some(parameters),
            active_parameter: active.map(to_u32),
        }],
        active_signature: Some(0),
        active_parameter: active.map(to_u32),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The signature and active parameter where `|` stands in `source`.
    fn help(source: &str) -> Option<(String, Option<u32>)> {
        let offset = source
            .find('|')
            .expect("the source marks the cursor with `|`");
        let document = Document::new(source.replacen('|', "", 1));
        let help = signature_help(&document, offset)?;
        Some((help.signatures[0].label.clone(), help.active_parameter))
    }

    #[test]
    fn shows_an_elements_parameters_and_the_one_being_filled() {
        let window = "window(title: fixed string, width: number, height: number)";
        assert_eq!(help("window(|"), Some((window.to_owned(), Some(0))));
        assert_eq!(help("window(\"App\", |"), Some((window.to_owned(), None)));
        assert_eq!(
            help("window(\"App\", height: |"),
            Some((window.to_owned(), Some(2)))
        );
    }

    #[test]
    fn shows_declared_callables_and_stays_inside_nested_brackets() {
        let source = "fn add(a: int, b: [int]) -> int { return a }\nlet x = add(1, [|";
        assert_eq!(
            help(source),
            Some(("add(a: int, b: [int])".to_owned(), Some(1)))
        );
    }

    #[test]
    fn shows_nothing_outside_a_call() {
        assert_eq!(help("state x = |"), None);
    }
}
