//! Go to definition, find references and highlighting the name under the cursor.

use lsp_types::{DocumentHighlight, Range};

use crate::document::{Document, Encoding};
use crate::index::{Role, SymbolId};

/// Where the name at `offset` is declared.
pub(crate) fn definition(document: &Document, offset: usize, encoding: Encoding) -> Option<Range> {
    let symbol = symbol_at(document, offset)?;
    let declaration = document.index.declaration_of(symbol)?;
    Some(document.range(declaration, encoding))
}

/// Every use of the name at `offset`, with its declaration when asked for.
pub(crate) fn references(
    document: &Document,
    offset: usize,
    include_declaration: bool,
    encoding: Encoding,
) -> Option<Vec<Range>> {
    let symbol = symbol_at(document, offset)?;
    let ranges = document
        .index
        .occurrences_of(symbol)
        .filter(|occurrence| include_declaration || matches!(occurrence.role, Role::Reference(_)))
        .map(|occurrence| document.range(occurrence.span, encoding))
        .collect();
    Some(ranges)
}

pub(crate) fn highlights(
    document: &Document,
    offset: usize,
    encoding: Encoding,
) -> Option<Vec<DocumentHighlight>> {
    let ranges = references(document, offset, true, encoding)?;
    let highlights = ranges
        .into_iter()
        .map(|range| DocumentHighlight { range, kind: None })
        .collect();
    Some(highlights)
}

fn symbol_at(document: &Document, offset: usize) -> Option<SymbolId> {
    document.index.occurrence_at(offset)?.role.symbol()
}

#[cfg(test)]
mod tests {
    use lsp_types::Position;

    use super::*;

    const SOURCE: &str = "state n = 0\nfn f() { n += 1 }";

    fn line_col(range: Range) -> (u32, u32) {
        (range.start.line, range.start.character)
    }

    #[test]
    fn jumps_from_a_reference_to_its_declaration() {
        let document = Document::new(SOURCE.to_owned());
        let use_site = document.offset(Position::new(1, 9), Encoding::Utf16);
        let found = definition(&document, use_site, Encoding::Utf16).map(line_col);
        assert_eq!(found, Some((0, 6)));
    }

    #[test]
    fn lists_references_with_or_without_the_declaration() {
        let document = Document::new(SOURCE.to_owned());
        let starts = |include_declaration| {
            references(&document, 6, include_declaration, Encoding::Utf16)
                .map(|ranges| ranges.into_iter().map(line_col).collect::<Vec<_>>())
        };
        assert_eq!(starts(true), Some(vec![(0, 6), (1, 9)]));
        assert_eq!(starts(false), Some(vec![(1, 9)]));
    }

    #[test]
    fn finds_nothing_for_built_ins() {
        let document = Document::new("label(\"hi\")".to_owned());
        assert_eq!(definition(&document, 2, Encoding::Utf16), None);
    }
}
