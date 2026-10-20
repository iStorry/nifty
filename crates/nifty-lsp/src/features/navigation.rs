//! Go to definition, find references and highlighting the name under the cursor. Top-level
//! names are shared by every file of a project, so definitions and references can be in
//! other files.

use lsp_types::{DocumentHighlight, Location};

use crate::document::Encoding;
use crate::index::{Occurrence, Role, SymbolId};
use crate::project::Cursor;

/// Where the name at the cursor is declared.
pub(crate) fn definition(cursor: Cursor, encoding: Encoding) -> Option<Location> {
    let symbol = symbol_at(cursor)?;
    let declaration = cursor.project.index.declaration_of(symbol)?;
    Some(cursor.project.location(declaration, encoding))
}

/// Every use of the name at the cursor, in any file, with its declaration when asked for.
pub(crate) fn references(
    cursor: Cursor,
    include_declaration: bool,
    encoding: Encoding,
) -> Option<Vec<Location>> {
    let locations = occurrences(cursor)?
        .filter(|occurrence| include_declaration || matches!(occurrence.role, Role::Reference(_)))
        .map(|occurrence| cursor.project.location(occurrence.span, encoding))
        .collect();
    Some(locations)
}

/// The declaration and uses of the name at the cursor within its own file.
pub(crate) fn highlights(cursor: Cursor, encoding: Encoding) -> Option<Vec<DocumentHighlight>> {
    let document = cursor.document();
    let highlights = occurrences(cursor)?
        .filter(|occurrence| occurrence.span.file == cursor.file)
        .map(|occurrence| DocumentHighlight {
            range: document.range(occurrence.span, encoding),
            kind: None,
        })
        .collect();
    Some(highlights)
}

fn occurrences(cursor: Cursor) -> Option<impl Iterator<Item = Occurrence>> {
    let symbol = symbol_at(cursor)?;
    Some(cursor.project.index.occurrences_of(symbol))
}

fn symbol_at(cursor: Cursor) -> Option<SymbolId> {
    cursor
        .project
        .index
        .occurrence_at(cursor.file, cursor.offset)?
        .role
        .symbol()
}

#[cfg(test)]
mod tests {
    use lsp_types::{Position, Range, Uri};
    use nifty_syntax::FileId;

    use super::*;
    use crate::project::{Project, Source, test_uri};

    const SOURCE: &str = "state n = 0\nfn f() { n += 1 }";

    fn start(location: Location) -> (Uri, u32, u32) {
        let Range { start, .. } = location.range;
        (location.uri, start.line, start.character)
    }

    fn cursor(project: &Project, file: usize, position: Position) -> Cursor<'_> {
        let file = FileId::new(file);
        let offset = project.document(file).offset(position, Encoding::Utf16);
        Cursor {
            project,
            file,
            offset,
        }
    }

    #[test]
    fn jumps_from_a_reference_to_its_declaration() {
        let project = Project::single(SOURCE);
        let found = definition(cursor(&project, 0, Position::new(1, 9)), Encoding::Utf16);
        assert_eq!(found.map(start), Some((test_uri("main.nf"), 0, 6)));
    }

    #[test]
    fn lists_references_with_or_without_the_declaration() {
        let project = Project::single(SOURCE);
        let starts = |include_declaration| {
            references(project.cursor(6), include_declaration, Encoding::Utf16).map(|found| {
                found
                    .into_iter()
                    .map(|location| {
                        let (_, line, column) = start(location);
                        (line, column)
                    })
                    .collect::<Vec<_>>()
            })
        };
        assert_eq!(starts(true), Some(vec![(0, 6), (1, 9)]));
        assert_eq!(starts(false), Some(vec![(1, 9)]));
    }

    #[test]
    fn follows_names_into_other_files() {
        let project = Project::new(vec![
            Source {
                uri: test_uri("main.nf"),
                text: "window(\"T\") {\n  Header()\n}".to_owned(),
            },
            Source {
                uri: test_uri("ui/header.nf"),
                text: "view Header() {}".to_owned(),
            },
        ]);
        let use_site = cursor(&project, 0, Position::new(1, 3));
        assert_eq!(
            definition(use_site, Encoding::Utf16).map(start),
            Some((test_uri("ui/header.nf"), 0, 5))
        );
        let everywhere = references(use_site, true, Encoding::Utf16)
            .map(|found| found.into_iter().map(start).collect::<Vec<_>>());
        assert_eq!(
            everywhere,
            Some(vec![
                (test_uri("main.nf"), 1, 2),
                (test_uri("ui/header.nf"), 0, 5)
            ])
        );
        let in_main = highlights(use_site, Encoding::Utf16).map(|found| found.len());
        assert_eq!(in_main, Some(1));
    }

    #[test]
    fn finds_nothing_for_built_ins() {
        let project = Project::single("label(\"hi\")");
        assert_eq!(definition(project.cursor(2), Encoding::Utf16), None);
    }
}
