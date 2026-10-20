//! The document outline: top-level declarations, struct fields, and top-level elements
//! such as windows.

use lsp_types::{DocumentSymbol, SymbolKind};
use nifty_syntax::Span;
use nifty_syntax::ast::{Arg, ExprKind, Ident, Item, Stmt, StmtKind, StringPart};

use crate::document::{Document, Encoding};

pub(crate) fn outline(document: &Document, encoding: Encoding) -> Vec<DocumentSymbol> {
    let outline = Outline { document, encoding };
    document
        .parsed
        .file
        .items
        .iter()
        .filter_map(|item| outline.item(item))
        .collect()
}

struct Outline<'doc> {
    document: &'doc Document,
    encoding: Encoding,
}

impl Outline<'_> {
    fn item(&self, item: &Item) -> Option<DocumentSymbol> {
        let symbol = match item {
            Item::State(decl) => self.declaration(&decl.name, SymbolKind::VARIABLE, decl.span),
            Item::Fn(decl) => self.declaration(&decl.name, SymbolKind::FUNCTION, decl.span),
            Item::View(decl) => self.declaration(&decl.name, SymbolKind::CLASS, decl.span),
            Item::Struct(decl) => {
                let fields = decl
                    .fields
                    .iter()
                    .map(|field| {
                        let span = field.name.span.to(field.ty.span);
                        self.declaration(&field.name, SymbolKind::FIELD, span)
                    })
                    .collect();
                DocumentSymbol {
                    children: Some(fields),
                    ..self.declaration(&decl.name, SymbolKind::STRUCT, decl.span)
                }
            }
            Item::Named(named) => self.element(&named.call, Some(&named.name))?,
            Item::Stmt(stmt) => self.element(stmt, None)?,
        };
        Some(symbol)
    }

    /// A top-level element such as a window, shown by its `name` if it has one, or else by
    /// its element, with its title as the detail.
    fn element(&self, stmt: &Stmt, name: Option<&Ident>) -> Option<DocumentSymbol> {
        let StmtKind::Expr(expr) = &stmt.kind else {
            return None;
        };
        let ExprKind::Call { callee, args, .. } = &expr.kind else {
            return None;
        };
        let ExprKind::Name(element) = &callee.kind else {
            return None;
        };
        let (shown, shown_span) =
            name.map_or((element, callee.span), |name| (&name.name, name.span));
        Some(DocumentSymbol {
            detail: title(args),
            ..self.symbol(shown, shown_span, SymbolKind::OBJECT, stmt.span)
        })
    }

    fn declaration(&self, name: &Ident, kind: SymbolKind, span: Span) -> DocumentSymbol {
        self.symbol(&name.name, name.span, kind, span)
    }

    // `DocumentSymbol::deprecated` is deprecated in favour of `tags` but has no default.
    #[allow(deprecated)]
    fn symbol(&self, name: &str, name_span: Span, kind: SymbolKind, span: Span) -> DocumentSymbol {
        DocumentSymbol {
            name: name.to_owned(),
            detail: None,
            kind,
            tags: None,
            deprecated: None,
            range: self.document.range(span, self.encoding),
            selection_range: self.document.range(name_span, self.encoding),
            children: None,
        }
    }
}

/// The first argument when it is a plain string, such as a window's title.
fn title(args: &[Arg]) -> Option<String> {
    let first = args.first().filter(|arg| arg.label.is_none())?;
    let ExprKind::String(parts) = &first.value.kind else {
        return None;
    };
    parts
        .iter()
        .map(|part| match part {
            StringPart::Text(text) => Some(text.as_str()),
            StringPart::Expr(_) => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use nifty_syntax::FileId;

    use super::*;

    #[test]
    fn lists_declarations_fields_and_windows() {
        let source = "struct P { x: int }\nstate n = 0\nwindow(\"App\") {}";
        let document = Document::new(source.to_owned(), FileId::default());
        let symbols = outline(&document, Encoding::Utf16);
        let summary: Vec<_> = symbols
            .iter()
            .map(|symbol| {
                let children: Vec<_> = symbol
                    .children
                    .iter()
                    .flatten()
                    .map(|child| child.name.as_str())
                    .collect();
                (symbol.name.as_str(), symbol.detail.as_deref(), children)
            })
            .collect();
        assert_eq!(
            summary,
            [
                ("P", None, vec!["x"]),
                ("n", None, vec![]),
                ("window", Some("App"), vec![])
            ]
        );
    }
}
