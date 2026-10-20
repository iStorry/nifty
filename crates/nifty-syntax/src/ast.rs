//! The syntax tree. It records what was written; names and types are checked later.

use crate::span::Span;
use crate::token::TokenKind;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct File {
    pub items: Vec<Item>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    State(StateDecl),
    Struct(StructDecl),
    Fn(FnDecl),
    View(ViewDecl),
    /// A top-level statement, such as a `window(...)` element.
    Stmt(Stmt),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

/// `name: Type`, as used by parameters and struct fields.
#[derive(Clone, Debug, PartialEq)]
pub struct TypedName {
    pub name: Ident,
    pub ty: TypeExpr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StateDecl {
    pub name: Ident,
    pub ty: Option<TypeExpr>,
    pub value: Expr,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StructDecl {
    pub name: Ident,
    pub fields: Vec<TypedName>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FnDecl {
    pub name: Ident,
    pub params: Vec<TypedName>,
    pub return_type: Option<TypeExpr>,
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewDecl {
    pub name: Ident,
    pub params: Vec<TypedName>,
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TypeExpr {
    pub kind: TypeExprKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TypeExprKind {
    Named(String),
    List(Box<TypeExpr>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StmtKind {
    Let {
        name: Ident,
        ty: Option<TypeExpr>,
        value: Expr,
    },
    Assign {
        target: Expr,
        op: AssignOp,
        value: Expr,
    },
    If(IfStmt),
    /// `for item in list`, `for index, item in list` or `for i in start..end`.
    For {
        index: Option<Ident>,
        item: Ident,
        iterable: Iterable,
        body: Block,
    },
    Return(Option<Expr>),
    Expr(Expr),
}

/// What a `for` loops over.
#[derive(Clone, Debug, PartialEq)]
pub enum Iterable {
    List(Expr),
    /// `start..end`: each `int` from `start` up to, but not including, `end`.
    Range {
        start: Expr,
        end: Expr,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct IfStmt {
    pub condition: Expr,
    pub then_block: Block,
    pub else_branch: Option<ElseBranch>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ElseBranch {
    If(Box<IfStmt>),
    Block(Block),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssignOp {
    Set,
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExprKind {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(Vec<StringPart>),
    Name(String),
    /// `.center`: a member of whatever type the context expects.
    ImplicitMember(Ident),
    List(Vec<Expr>),
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Arg>,
        trailing_block: Option<Block>,
    },
    Field {
        base: Box<Expr>,
        name: Ident,
    },
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
    },
    /// Stands in for an expression that failed to parse, so parsing can continue.
    Error,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StringPart {
    Text(String),
    Expr(Expr),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Arg {
    pub label: Option<Ident>,
    pub value: Expr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
    Neg,
    /// `$place`: a two-way binding.
    Bind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOp {
    Or,
    And,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

impl BinaryOp {
    #[must_use]
    pub fn is_comparison(self) -> bool {
        matches!(
            self,
            Self::Eq | Self::NotEq | Self::Lt | Self::LtEq | Self::Gt | Self::GtEq
        )
    }

    /// The operator token, which also gives its spelling.
    #[must_use]
    pub const fn token(self) -> TokenKind {
        match self {
            Self::Or => TokenKind::OrOr,
            Self::And => TokenKind::AndAnd,
            Self::Eq => TokenKind::EqEq,
            Self::NotEq => TokenKind::BangEq,
            Self::Lt => TokenKind::Lt,
            Self::LtEq => TokenKind::LtEq,
            Self::Gt => TokenKind::Gt,
            Self::GtEq => TokenKind::GtEq,
            Self::Add => TokenKind::Plus,
            Self::Sub => TokenKind::Minus,
            Self::Mul => TokenKind::Star,
            Self::Div => TokenKind::Slash,
            Self::Rem => TokenKind::Percent,
        }
    }
}
