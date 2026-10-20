use std::borrow::Cow;

use crate::span::Span;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Ident,
    Int,
    Float,

    /// A string is `StringStart (StringText | InterpolationStart … InterpolationEnd)* StringEnd`.
    StringStart,
    StringText,
    InterpolationStart,
    InterpolationEnd,
    StringEnd,

    State,
    Let,
    Fn,
    View,
    Struct,
    If,
    Else,
    For,
    In,
    Return,
    True,
    False,

    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Colon,
    Dot,
    Arrow,
    Dollar,

    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Bang,
    Eq,
    EqEq,
    BangEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    AndAnd,
    OrOr,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,

    Newline,
    Eof,
}

/// Every keyword; [`TokenKind::text`] spells them.
pub const KEYWORDS: &[TokenKind] = &[
    TokenKind::State,
    TokenKind::Let,
    TokenKind::Fn,
    TokenKind::View,
    TokenKind::Struct,
    TokenKind::If,
    TokenKind::Else,
    TokenKind::For,
    TokenKind::In,
    TokenKind::Return,
    TokenKind::True,
    TokenKind::False,
];

/// Every punctuation and operator token; [`TokenKind::text`] spells them.
pub const SYMBOLS: &[TokenKind] = &[
    TokenKind::LParen,
    TokenKind::RParen,
    TokenKind::LBrace,
    TokenKind::RBrace,
    TokenKind::LBracket,
    TokenKind::RBracket,
    TokenKind::Comma,
    TokenKind::Colon,
    TokenKind::Dot,
    TokenKind::Arrow,
    TokenKind::Dollar,
    TokenKind::Plus,
    TokenKind::Minus,
    TokenKind::Star,
    TokenKind::Slash,
    TokenKind::Percent,
    TokenKind::Bang,
    TokenKind::Eq,
    TokenKind::EqEq,
    TokenKind::BangEq,
    TokenKind::Lt,
    TokenKind::LtEq,
    TokenKind::Gt,
    TokenKind::GtEq,
    TokenKind::AndAnd,
    TokenKind::OrOr,
    TokenKind::PlusEq,
    TokenKind::MinusEq,
    TokenKind::StarEq,
    TokenKind::SlashEq,
];

impl TokenKind {
    pub(crate) fn keyword(text: &str) -> Option<Self> {
        KEYWORDS
            .iter()
            .copied()
            .find(|kind| kind.text() == Some(text))
    }

    /// How a keyword, punctuation or operator token is spelled, or `None` for tokens
    /// whose text varies, such as names and numbers.
    #[must_use]
    pub const fn text(self) -> Option<&'static str> {
        match self.spelling() {
            Spelling::Fixed(text) => Some(text),
            Spelling::Varies(_) => None,
        }
    }

    /// How the token reads in an error message.
    #[must_use]
    pub fn describe(self) -> Cow<'static, str> {
        match self.spelling() {
            Spelling::Fixed(text) => Cow::Owned(format!("`{text}`")),
            Spelling::Varies(description) => Cow::Borrowed(description),
        }
    }

    const fn spelling(self) -> Spelling {
        use Spelling::{Fixed, Varies};
        match self {
            Self::Ident => Varies("a name"),
            Self::Int | Self::Float => Varies("a number"),
            Self::StringStart => Varies("a string"),
            Self::StringText | Self::StringEnd => Varies("string text"),
            Self::InterpolationStart => Varies("`{` in a string"),
            Self::InterpolationEnd => Varies("`}` in a string"),
            Self::Newline => Varies("end of line"),
            Self::Eof => Varies("end of file"),
            Self::State => Fixed("state"),
            Self::Let => Fixed("let"),
            Self::Fn => Fixed("fn"),
            Self::View => Fixed("view"),
            Self::Struct => Fixed("struct"),
            Self::If => Fixed("if"),
            Self::Else => Fixed("else"),
            Self::For => Fixed("for"),
            Self::In => Fixed("in"),
            Self::Return => Fixed("return"),
            Self::True => Fixed("true"),
            Self::False => Fixed("false"),
            Self::LParen => Fixed("("),
            Self::RParen => Fixed(")"),
            Self::LBrace => Fixed("{"),
            Self::RBrace => Fixed("}"),
            Self::LBracket => Fixed("["),
            Self::RBracket => Fixed("]"),
            Self::Comma => Fixed(","),
            Self::Colon => Fixed(":"),
            Self::Dot => Fixed("."),
            Self::Arrow => Fixed("->"),
            Self::Dollar => Fixed("$"),
            Self::Plus => Fixed("+"),
            Self::Minus => Fixed("-"),
            Self::Star => Fixed("*"),
            Self::Slash => Fixed("/"),
            Self::Percent => Fixed("%"),
            Self::Bang => Fixed("!"),
            Self::Eq => Fixed("="),
            Self::EqEq => Fixed("=="),
            Self::BangEq => Fixed("!="),
            Self::Lt => Fixed("<"),
            Self::LtEq => Fixed("<="),
            Self::Gt => Fixed(">"),
            Self::GtEq => Fixed(">="),
            Self::AndAnd => Fixed("&&"),
            Self::OrOr => Fixed("||"),
            Self::PlusEq => Fixed("+="),
            Self::MinusEq => Fixed("-="),
            Self::StarEq => Fixed("*="),
            Self::SlashEq => Fixed("/="),
        }
    }
}

/// A token's text, or for tokens whose text varies, how an error message names them.
enum Spelling {
    Fixed(&'static str),
    Varies(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}
