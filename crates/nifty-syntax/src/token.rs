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

pub(crate) const KEYWORDS: &[(&str, TokenKind)] = &[
    ("state", TokenKind::State),
    ("let", TokenKind::Let),
    ("fn", TokenKind::Fn),
    ("view", TokenKind::View),
    ("struct", TokenKind::Struct),
    ("if", TokenKind::If),
    ("else", TokenKind::Else),
    ("for", TokenKind::For),
    ("in", TokenKind::In),
    ("return", TokenKind::Return),
    ("true", TokenKind::True),
    ("false", TokenKind::False),
];

impl TokenKind {
    pub(crate) fn keyword(text: &str) -> Option<Self> {
        KEYWORDS
            .iter()
            .find(|(keyword, _)| *keyword == text)
            .map(|&(_, kind)| kind)
    }

    /// How a keyword is spelled, or `None` for any other token.
    #[must_use]
    pub fn keyword_text(self) -> Option<&'static str> {
        KEYWORDS
            .iter()
            .find(|&&(_, kind)| kind == self)
            .map(|&(text, _)| text)
    }

    /// How the token reads in an error message.
    #[must_use]
    pub fn describe(self) -> &'static str {
        match self {
            Self::Ident => "a name",
            Self::Int | Self::Float => "a number",
            Self::StringStart => "a string",
            Self::StringText | Self::StringEnd => "string text",
            Self::InterpolationStart => "`{` in a string",
            Self::InterpolationEnd => "`}` in a string",
            Self::State => "`state`",
            Self::Let => "`let`",
            Self::Fn => "`fn`",
            Self::View => "`view`",
            Self::Struct => "`struct`",
            Self::If => "`if`",
            Self::Else => "`else`",
            Self::For => "`for`",
            Self::In => "`in`",
            Self::Return => "`return`",
            Self::True => "`true`",
            Self::False => "`false`",
            Self::LParen => "`(`",
            Self::RParen => "`)`",
            Self::LBrace => "`{`",
            Self::RBrace => "`}`",
            Self::LBracket => "`[`",
            Self::RBracket => "`]`",
            Self::Comma => "`,`",
            Self::Colon => "`:`",
            Self::Dot => "`.`",
            Self::Arrow => "`->`",
            Self::Dollar => "`$`",
            Self::Plus => "`+`",
            Self::Minus => "`-`",
            Self::Star => "`*`",
            Self::Slash => "`/`",
            Self::Percent => "`%`",
            Self::Bang => "`!`",
            Self::Eq => "`=`",
            Self::EqEq => "`==`",
            Self::BangEq => "`!=`",
            Self::Lt => "`<`",
            Self::LtEq => "`<=`",
            Self::Gt => "`>`",
            Self::GtEq => "`>=`",
            Self::AndAnd => "`&&`",
            Self::OrOr => "`||`",
            Self::PlusEq => "`+=`",
            Self::MinusEq => "`-=`",
            Self::StarEq => "`*=`",
            Self::SlashEq => "`/=`",
            Self::Newline => "end of line",
            Self::Eof => "end of file",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}
