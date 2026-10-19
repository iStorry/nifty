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

impl TokenKind {
    pub(crate) fn keyword(text: &str) -> Option<Self> {
        Some(match text {
            "state" => Self::State,
            "let" => Self::Let,
            "fn" => Self::Fn,
            "view" => Self::View,
            "struct" => Self::Struct,
            "if" => Self::If,
            "else" => Self::Else,
            "for" => Self::For,
            "in" => Self::In,
            "return" => Self::Return,
            "true" => Self::True,
            "false" => Self::False,
            _ => return None,
        })
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
