//! Turns source text into tokens. Strings are split into text and interpolation tokens so
//! the parser handles `"{expr}"` with the ordinary expression grammar.

use crate::diagnostic::Diagnostic;
use crate::span::Span;
use crate::token::{Token, TokenKind};

pub(crate) fn lex(source: &str) -> (Vec<Token>, Vec<Diagnostic>) {
    let mut lexer = Lexer {
        source,
        bytes: source.as_bytes(),
        pos: 0,
        modes: vec![Mode::Code { open_braces: 0 }],
        tokens: Vec::new(),
        diagnostics: Vec::new(),
    };
    lexer.run();
    (lexer.tokens, lexer.diagnostics)
}

/// The text of a `StringText` token with its escapes applied.
pub(crate) fn unescape(raw: &str) -> String {
    let mut text = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            text.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => text.push('\n'),
            Some('t') => text.push('\t'),
            Some(escaped) => text.push(escaped),
            None => {}
        }
    }
    text
}

/// `text` written as the inside of a string literal, so [`unescape`] gives it back.
pub(crate) fn escape(text: &str) -> String {
    let mut raw = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\n' => raw.push_str("\\n"),
            '\t' => raw.push_str("\\t"),
            '"' | '\\' | '{' | '}' => {
                raw.push('\\');
                raw.push(c);
            }
            _ => raw.push(c),
        }
    }
    raw
}

/// The characters that may follow `\\` in a string.
pub const ESCAPES: &[char] = &['n', 't', '"', '\\', '{', '}'];

enum Mode {
    /// `open_braces` counts `{` inside an interpolation, so its closing `}` can be found.
    Code {
        open_braces: u32,
    },
    String {
        start: usize,
    },
}

struct Lexer<'src> {
    source: &'src str,
    bytes: &'src [u8],
    pos: usize,
    modes: Vec<Mode>,
    tokens: Vec<Token>,
    diagnostics: Vec<Diagnostic>,
}

impl Lexer<'_> {
    fn run(&mut self) {
        while self.pos < self.bytes.len() {
            match self.modes.last() {
                Some(Mode::String { start }) => self.lex_string_part(*start),
                _ => self.lex_code(),
            }
        }
        if let Some(Mode::String { start }) = self
            .modes
            .iter()
            .rev()
            .find(|mode| matches!(mode, Mode::String { .. }))
        {
            let span = Span::new(*start, self.pos);
            self.error("this string is never closed", span);
        }
        self.push(TokenKind::Eof, self.pos);
    }

    fn lex_code(&mut self) {
        let start = self.pos;
        let byte = self.bytes[start];
        self.pos += 1;
        let kind = match byte {
            b' ' | b'\t' | b'\r' => return,
            b'\n' => {
                if self
                    .tokens
                    .last()
                    .is_some_and(|t| t.kind != TokenKind::Newline)
                {
                    self.push(TokenKind::Newline, start);
                }
                return;
            }
            b'/' if self.peek() == Some(b'/') => {
                while self.peek().is_some_and(|b| b != b'\n') {
                    self.pos += 1;
                }
                return;
            }
            b'"' => {
                self.push(TokenKind::StringStart, start);
                self.modes.push(Mode::String { start });
                return;
            }
            b'0'..=b'9' => self.number(),
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => self.identifier(start),
            b'{' => {
                if let Some(Mode::Code { open_braces }) = self.modes.last_mut() {
                    *open_braces += 1;
                }
                TokenKind::LBrace
            }
            b'}' => self.close_brace(),
            b'(' => TokenKind::LParen,
            b')' => TokenKind::RParen,
            b'[' => TokenKind::LBracket,
            b']' => TokenKind::RBracket,
            b',' => TokenKind::Comma,
            b':' => TokenKind::Colon,
            b'.' => TokenKind::Dot,
            b'$' => TokenKind::Dollar,
            b'%' => TokenKind::Percent,
            b'+' => self.with_eq(TokenKind::Plus, TokenKind::PlusEq),
            b'*' => self.with_eq(TokenKind::Star, TokenKind::StarEq),
            b'/' => self.with_eq(TokenKind::Slash, TokenKind::SlashEq),
            b'=' => self.with_eq(TokenKind::Eq, TokenKind::EqEq),
            b'!' => self.with_eq(TokenKind::Bang, TokenKind::BangEq),
            b'<' => self.with_eq(TokenKind::Lt, TokenKind::LtEq),
            b'>' => self.with_eq(TokenKind::Gt, TokenKind::GtEq),
            b'-' if self.eat(b'>') => TokenKind::Arrow,
            b'-' => self.with_eq(TokenKind::Minus, TokenKind::MinusEq),
            b'&' | b'|' => {
                if self.eat(byte) {
                    if byte == b'&' {
                        TokenKind::AndAnd
                    } else {
                        TokenKind::OrOr
                    }
                } else {
                    let doubled = if byte == b'&' { "&&" } else { "||" };
                    self.error(format!("expected `{doubled}`"), Span::new(start, self.pos));
                    return;
                }
            }
            _ => {
                self.unexpected_character(start);
                return;
            }
        };
        self.push(kind, start);
    }

    fn close_brace(&mut self) -> TokenKind {
        let in_interpolation = self.modes.len() > 1;
        match self.modes.last_mut() {
            Some(Mode::Code { open_braces: 0 }) if in_interpolation => {
                self.modes.pop();
                TokenKind::InterpolationEnd
            }
            Some(Mode::Code { open_braces }) => {
                *open_braces = open_braces.saturating_sub(1);
                TokenKind::RBrace
            }
            _ => TokenKind::RBrace,
        }
    }

    fn lex_string_part(&mut self, string_start: usize) {
        let start = self.pos;
        match self.bytes[start] {
            b'"' => {
                self.pos += 1;
                self.modes.pop();
                self.push(TokenKind::StringEnd, start);
            }
            b'{' => {
                self.pos += 1;
                self.push(TokenKind::InterpolationStart, start);
                self.modes.push(Mode::Code { open_braces: 0 });
            }
            b'\n' => {
                self.error(
                    "this string is never closed",
                    Span::new(string_start, start),
                );
                self.modes.pop();
                self.push(TokenKind::StringEnd, start);
            }
            _ => {
                while let Some(byte) = self.peek() {
                    match byte {
                        b'"' | b'{' | b'\n' => break,
                        b'\\' => self.escape(),
                        _ => self.pos += 1,
                    }
                }
                self.push(TokenKind::StringText, start);
            }
        }
    }

    fn escape(&mut self) {
        let start = self.pos;
        self.pos += 1;
        let Some(escaped) = self.source[self.pos..].chars().next() else {
            return;
        };
        if escaped == '\n' {
            return;
        }
        self.pos += escaped.len_utf8();
        if !ESCAPES.contains(&escaped) {
            let valid: Vec<_> = ESCAPES.iter().map(|c| format!("\\{c}")).collect();
            self.error(
                format!("unknown escape `\\{escaped}`; use {}", valid.join(" ")),
                Span::new(start, self.pos),
            );
        }
    }

    fn number(&mut self) -> TokenKind {
        self.digits();
        let fraction_follows = self.peek() == Some(b'.')
            && self.bytes.get(self.pos + 1).is_some_and(u8::is_ascii_digit);
        if !fraction_follows {
            return TokenKind::Int;
        }
        self.pos += 1;
        self.digits();
        TokenKind::Float
    }

    fn digits(&mut self) {
        while self.peek().is_some_and(|b| b.is_ascii_digit() || b == b'_') {
            self.pos += 1;
        }
    }

    fn identifier(&mut self, start: usize) -> TokenKind {
        while self
            .peek()
            .is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            self.pos += 1;
        }
        TokenKind::keyword(&self.source[start..self.pos]).unwrap_or(TokenKind::Ident)
    }

    fn unexpected_character(&mut self, start: usize) {
        let Some(c) = self.source[start..].chars().next() else {
            return;
        };
        self.pos = start + c.len_utf8();
        self.error(
            format!("unexpected character `{c}`"),
            Span::new(start, self.pos),
        );
    }

    fn with_eq(&mut self, single: TokenKind, with_eq: TokenKind) -> TokenKind {
        if self.eat(b'=') { with_eq } else { single }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn eat(&mut self, expected: u8) -> bool {
        let matched = self.peek() == Some(expected);
        if matched {
            self.pos += 1;
        }
        matched
    }

    fn push(&mut self, kind: TokenKind, start: usize) {
        self.tokens.push(Token {
            kind,
            span: Span::new(start, self.pos),
        });
    }

    fn error(&mut self, message: impl Into<String>, span: Span) {
        self.diagnostics.push(Diagnostic::new(message, span));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::{KEYWORDS, SYMBOLS};
    use TokenKind::*;

    fn kinds(source: &str) -> Vec<TokenKind> {
        let (tokens, diagnostics) = lex(source);
        assert_eq!(diagnostics, []);
        tokens.into_iter().map(|token| token.kind).collect()
    }

    #[test]
    fn splits_interpolated_strings() {
        assert_eq!(
            kinds(r#""a {b + 1} c""#),
            [
                StringStart,
                StringText,
                InterpolationStart,
                Ident,
                Plus,
                Int,
                InterpolationEnd,
                StringText,
                StringEnd,
                Eof
            ]
        );
    }

    #[test]
    fn braces_inside_interpolation_do_not_end_it() {
        assert_eq!(
            kinds(r#""{f() { x }}""#),
            [
                StringStart,
                InterpolationStart,
                Ident,
                LParen,
                RParen,
                LBrace,
                Ident,
                RBrace,
                InterpolationEnd,
                StringEnd,
                Eof
            ]
        );
    }

    #[test]
    fn collapses_blank_lines_and_skips_comments() {
        assert_eq!(kinds("a // note\n\n\nb"), [Ident, Newline, Ident, Eof]);
    }

    #[test]
    fn reads_numbers_operators_and_keywords() {
        assert_eq!(
            kinds("state x = 1_000 + 2.5 -> a.len() != -3"),
            [
                State, Ident, Eq, Int, Plus, Float, Arrow, Ident, Dot, Ident, LParen, RParen,
                BangEq, Minus, Int, Eof
            ]
        );
    }

    #[test]
    fn every_keyword_and_symbol_lexes_to_its_token() {
        for &kind in KEYWORDS.iter().chain(SYMBOLS) {
            let text = kind.text().expect("keywords and symbols have fixed text");
            assert_eq!(kinds(text), [kind, Eof], "lexing `{text}`");
        }
    }

    #[test]
    fn unescapes_string_text() {
        let source = r#""say \"hi\"\n\{x\}""#;
        let (tokens, diagnostics) = lex(source);
        assert_eq!(diagnostics, []);
        assert_eq!(unescape(&source[tokens[1].span.range()]), "say \"hi\"\n{x}");
    }

    #[test]
    fn escaped_text_lexes_cleanly_and_unescapes_back() {
        let text = "C:\\Users\\me\t\"{x}\"\n";
        let source = format!("\"{}\"", escape(text));
        let (tokens, diagnostics) = lex(&source);
        assert_eq!(diagnostics, []);
        assert_eq!(unescape(&source[tokens[1].span.range()]), text);
    }

    #[test]
    fn reports_unknown_escapes_and_unclosed_strings() {
        let (_, diagnostics) = lex("\"a\\qb\nx");
        let messages: Vec<_> = diagnostics.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(
            messages,
            [
                "unknown escape `\\q`; use \\n \\t \\\" \\\\ \\{ \\}",
                "this string is never closed"
            ]
        );
    }
}
