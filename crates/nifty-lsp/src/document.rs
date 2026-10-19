//! An open source file: its text, its syntax tree and symbol index, and conversion between
//! byte offsets and LSP positions.

use lsp_types::{ClientCapabilities, Position, PositionEncodingKind, Range};
use nifty_syntax::{Parsed, Span};

use crate::index::Index;

/// The unit LSP positions count columns in, agreed with the client at startup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Encoding {
    Utf8,
    Utf16,
}

impl Encoding {
    /// UTF-8 matches nifty's byte spans, so it is used when the client offers it. UTF-16
    /// is the protocol's default and every client supports it.
    pub(crate) fn negotiate(capabilities: &ClientCapabilities) -> Self {
        let offers_utf8 = capabilities
            .general
            .as_ref()
            .and_then(|general| general.position_encodings.as_ref())
            .is_some_and(|encodings| encodings.contains(&PositionEncodingKind::UTF8));
        if offers_utf8 { Self::Utf8 } else { Self::Utf16 }
    }

    pub(crate) fn kind(self) -> PositionEncodingKind {
        match self {
            Self::Utf8 => PositionEncodingKind::UTF8,
            Self::Utf16 => PositionEncodingKind::UTF16,
        }
    }

    fn units(self, c: char) -> usize {
        match self {
            Self::Utf8 => c.len_utf8(),
            Self::Utf16 => c.len_utf16(),
        }
    }

    pub(crate) fn len(self, text: &str) -> u32 {
        to_u32(text.chars().map(|c| self.units(c)).sum())
    }
}

#[derive(Debug)]
pub(crate) struct Document {
    text: String,
    /// The byte offset where each line starts; the first is always 0.
    line_starts: Vec<usize>,
    pub version: i32,
    pub parsed: Parsed,
    pub index: Index,
}

impl Document {
    pub(crate) fn new(text: String, version: i32) -> Self {
        let line_starts = std::iter::once(0)
            .chain(text.match_indices('\n').map(|(newline, _)| newline + 1))
            .collect();
        let parsed = nifty_syntax::parse(&text);
        let index = Index::new(&parsed.file);
        Self {
            text,
            line_starts,
            version,
            parsed,
            index,
        }
    }

    pub(crate) fn text(&self, span: Span) -> &str {
        &self.text[span.range()]
    }

    pub(crate) fn range(&self, span: Span, encoding: Encoding) -> Range {
        Range::new(
            self.position(span.start, encoding),
            self.position(span.end, encoding),
        )
    }

    pub(crate) fn position(&self, offset: usize, encoding: Encoding) -> Position {
        let line = self.line_starts.partition_point(|&start| start <= offset) - 1;
        let line_text = &self.text[self.line_starts[line]..offset];
        Position::new(to_u32(line), encoding.len(line_text))
    }

    /// The byte offset of `position`. A column past the end of its line means the end of
    /// the line, and a line past the end of the file means the end of the file, as the
    /// protocol requires.
    pub(crate) fn offset(&self, position: Position, encoding: Encoding) -> usize {
        let Some(&line_start) = self.line_starts.get(position.line as usize) else {
            return self.text.len();
        };
        let line_end = self.text[line_start..]
            .find('\n')
            .map_or(self.text.len(), |newline| line_start + newline);
        let mut column = 0;
        for (index, c) in self.text[line_start..line_end].char_indices() {
            if column >= position.character as usize {
                return line_start + index;
            }
            column += encoding.units(c);
        }
        line_end
    }
}

/// LSP counts in `u32`; a source file never comes near that size.
fn to_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_between_offsets_and_positions_in_both_encodings() {
        // `é` is 2 bytes and 1 UTF-16 unit; `𝄞` is 4 bytes and 2 UTF-16 units.
        let document = Document::new("a\n\"é𝄞\" x".to_owned(), 0);
        let x = document.text.find('x').unwrap_or_default();
        assert_eq!(document.position(x, Encoding::Utf8), Position::new(1, 9));
        assert_eq!(document.position(x, Encoding::Utf16), Position::new(1, 6));
        assert_eq!(document.offset(Position::new(1, 9), Encoding::Utf8), x);
        assert_eq!(document.offset(Position::new(1, 6), Encoding::Utf16), x);
    }

    #[test]
    fn clamps_positions_past_the_end_of_a_line_or_file() {
        let document = Document::new("ab\ncd".to_owned(), 0);
        assert_eq!(document.offset(Position::new(0, 99), Encoding::Utf16), 2);
        assert_eq!(document.offset(Position::new(9, 0), Encoding::Utf16), 5);
    }
}
