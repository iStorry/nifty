//! Recursive-descent parser. On an error it records a diagnostic, skips to the end of the
//! line and carries on, so one mistake reports once instead of cascading.

mod expr;
mod item;
mod stmt;
#[cfg(test)]
mod tests;

use crate::ast::{File, Ident};
use crate::diagnostic::Diagnostic;
use crate::lexer;
use crate::span::Span;
use crate::token::{Token, TokenKind};

/// The diagnostic is already recorded; the caller should recover.
#[derive(Debug)]
struct Reported;

type ParseResult<T> = Result<T, Reported>;

/// Lexes and parses `source`, returning the tree and every diagnostic in source order.
pub(crate) fn parse(source: &str) -> (File, Vec<Diagnostic>) {
    let (tokens, mut diagnostics) = lexer::lex(source);
    let mut parser = Parser {
        source,
        tokens,
        pos: 0,
        diagnostics: Vec::new(),
    };
    let file = parser.parse_file();
    diagnostics.extend(parser.diagnostics);
    diagnostics.sort_by_key(|diagnostic| diagnostic.span.start);
    (file, diagnostics)
}

struct Parser<'src> {
    source: &'src str,
    /// Always ends with `Eof`, and `bump` never moves past it.
    tokens: Vec<Token>,
    pos: usize,
    diagnostics: Vec<Diagnostic>,
}

impl Parser<'_> {
    fn parse_file(&mut self) -> File {
        let mut items = Vec::new();
        loop {
            self.skip_newlines();
            match self.peek() {
                TokenKind::Eof => return File { items },
                TokenKind::RBrace => {
                    let token = self.bump();
                    self.report("this `}` has no matching `{`", token.span);
                }
                _ => match self.parse_item() {
                    Ok(item) => {
                        items.push(item);
                        self.finish_line();
                    }
                    Err(Reported) => self.recover_to_line_end(),
                },
            }
        }
    }

    fn current(&self) -> Token {
        self.tokens[self.pos]
    }

    fn peek(&self) -> TokenKind {
        self.current().kind
    }

    fn peek_nth(&self, n: usize) -> TokenKind {
        self.tokens
            .get(self.pos + n)
            .map_or(TokenKind::Eof, |token| token.kind)
    }

    fn at(&self, kind: TokenKind) -> bool {
        self.peek() == kind
    }

    fn bump(&mut self) -> Token {
        let token = self.current();
        if token.kind != TokenKind::Eof {
            self.pos += 1;
        }
        token
    }

    fn eat(&mut self, kind: TokenKind) -> Option<Token> {
        self.at(kind).then(|| self.bump())
    }

    fn expect(&mut self, kind: TokenKind) -> ParseResult<Token> {
        if let Some(token) = self.eat(kind) {
            return Ok(token);
        }
        self.unexpected(kind.describe())
    }

    fn expect_ident(&mut self) -> ParseResult<Ident> {
        let token = self.expect(TokenKind::Ident)?;
        Ok(Ident {
            name: self.text(token.span).to_owned(),
            span: token.span,
        })
    }

    /// Reports "expected {expected}, found …" at the current token.
    fn unexpected<T>(&mut self, expected: &str) -> ParseResult<T> {
        let found = self.current();
        Err(self.report(
            format!("expected {expected}, found {}", found.kind.describe()),
            found.span,
        ))
    }

    fn report(&mut self, message: impl Into<String>, span: Span) -> Reported {
        self.diagnostics.push(Diagnostic::new(message, span));
        Reported
    }

    fn text(&self, span: Span) -> &str {
        &self.source[span.range()]
    }

    fn skip_newlines(&mut self) {
        while self.eat(TokenKind::Newline).is_some() {}
    }

    /// The span from `start` to the end of the last consumed token.
    fn span_from(&self, start: usize) -> Span {
        let end = self
            .pos
            .checked_sub(1)
            .map_or(start, |previous| self.tokens[previous].span.end);
        Span::new(start, end)
    }

    /// A statement must end at a line break, a closing `}` or the end of the file.
    fn finish_line(&mut self) {
        match self.peek() {
            TokenKind::Newline => {
                self.bump();
            }
            TokenKind::RBrace | TokenKind::Eof => {}
            _ => {
                let token = self.current();
                self.report(
                    format!(
                        "expected end of line, found {}; put each statement on its own line",
                        token.kind.describe()
                    ),
                    token.span,
                );
                self.recover_to_line_end();
            }
        }
    }

    /// Skips to the next line break or the `}` closing the current block, ignoring any
    /// nested in brackets along the way.
    fn recover_to_line_end(&mut self) {
        let mut depth = 0usize;
        loop {
            let kind = self.peek();
            match kind {
                TokenKind::Eof => return,
                TokenKind::Newline | TokenKind::RBrace if depth == 0 => return,
                TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace => depth += 1,
                TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace => {
                    depth = depth.saturating_sub(1);
                }
                _ => {}
            }
            self.bump();
        }
    }

    /// Parses `item (, item)* ,? close` after the opening bracket; newlines are free inside.
    fn comma_list<T>(
        &mut self,
        close: TokenKind,
        mut item: impl FnMut(&mut Self) -> ParseResult<T>,
    ) -> ParseResult<Vec<T>> {
        let mut items = Vec::new();
        loop {
            self.skip_newlines();
            if self.eat(close).is_some() {
                return Ok(items);
            }
            items.push(item(self)?);
            self.skip_newlines();
            if self.eat(TokenKind::Comma).is_none() {
                self.expect(close)?;
                return Ok(items);
            }
        }
    }
}
