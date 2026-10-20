//! What the name under the cursor is: a callable's signature and, for built-ins, the
//! summary from their table.

use lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind};
use nifty_check::methods;

use crate::document::Encoding;
use crate::index::{Role, SymbolKind, Unresolved};
use crate::project::{Cursor, Project};
use crate::signature;

pub(crate) fn hover(cursor: Cursor, encoding: Encoding) -> Option<Hover> {
    let project = cursor.project;
    let occurrence = project.index.occurrence_at(cursor.file, cursor.offset)?;
    let name = project.text(occurrence.span);
    let (code, summary) = match occurrence.role {
        Role::Unresolved(Unresolved::Method) => {
            let method = methods::method(name)?;
            let code = format!("{}({})", method.name, method.params.join(", "));
            (code, Some(method.summary))
        }
        Role::Unresolved(Unresolved::Function) => callable(project, name)?,
        Role::Declaration(id) | Role::Reference(id)
            if matches!(
                project.index.symbol(id).kind,
                SymbolKind::Function | SymbolKind::View | SymbolKind::Struct
            ) =>
        {
            callable(project, name)?
        }
        _ => return None,
    };
    let mut markdown = format!("```nifty\n{code}\n```");
    if let Some(summary) = summary {
        markdown.push_str("\n\n");
        markdown.push_str(summary);
    }
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: markdown,
        }),
        range: Some(cursor.document().range(occurrence.span, encoding)),
    })
}

fn callable(project: &Project, name: &str) -> Option<(String, Option<&'static str>)> {
    let signature = signature::of(project, name)?;
    Some((signature.label().0, signature.summary))
}

#[cfg(test)]
mod tests {
    use nifty_check::elements;
    use nifty_check::functions::FUNCTIONS;
    use nifty_check::methods::METHODS;

    use super::*;

    /// The hover text where `|` stands in `source`.
    fn text(source: &str) -> Option<String> {
        let (project, offset) = Project::with_cursor(source);
        let hover = hover(project.cursor(offset), Encoding::Utf16)?;
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("hover is markdown");
        };
        Some(markup.value)
    }

    #[test]
    fn built_ins_show_their_signature_and_summary() {
        let label = elements::element("label").expect("label is an element");
        let shown = text("window(\"A\") {\n  la|bel(\"hi\")\n}").expect("a hover");
        assert!(shown.starts_with("```nifty\nlabel(text: string"), "{shown}");
        assert!(shown.ends_with(label.summary), "{shown}");

        let download = &FUNCTIONS[0];
        let shown = text(&format!("fn f() {{\n  {}|(\"u\")\n}}", download.name)).expect("a hover");
        assert!(shown.ends_with(download.summary), "{shown}");
    }

    #[test]
    fn methods_show_their_parameters_and_summary() {
        let push = METHODS
            .iter()
            .find(|method| method.name == "push")
            .expect("push is a method");
        let shown = text("state xs: [int] = []\nfn f() {\n  xs.pu|sh(1)\n}").expect("a hover");
        assert_eq!(
            shown,
            format!("```nifty\npush(item)\n```\n\n{}", push.summary)
        );
    }

    #[test]
    fn declared_callables_show_their_signature() {
        let shown = text("fn add(a: int) -> int { return a }\nlet x = ad|d(1)");
        assert_eq!(shown.as_deref(), Some("```nifty\nadd(a: int)\n```"));
    }

    #[test]
    fn values_and_unknown_names_show_nothing() {
        assert_eq!(text("state count = 0\nlet x = cou|nt"), None);
        assert_eq!(text("fn f() {\n  nope|()\n}"), None);
    }
}
