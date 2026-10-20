//! Expressions, parsed by precedence climbing.

use std::str::FromStr;

use super::{ParseResult, Parser};
use crate::ast::{Arg, BinaryOp, Expr, ExprKind, StringPart, UnaryOp};
use crate::lexer::unescape;
use crate::span::Span;
use crate::token::TokenKind;

/// Whether `call() { … }` may take a trailing block here. It is forbidden where a `{`
/// opens the statement's own block, as in `if ready() { … }`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TrailingBlocks {
    Allowed,
    Forbidden,
}

/// Binding power of each binary operator; higher binds tighter.
fn binding_power(op: BinaryOp) -> u8 {
    match op {
        BinaryOp::Or => 1,
        BinaryOp::And => 2,
        BinaryOp::Eq
        | BinaryOp::NotEq
        | BinaryOp::Lt
        | BinaryOp::LtEq
        | BinaryOp::Gt
        | BinaryOp::GtEq => 3,
        BinaryOp::Add | BinaryOp::Sub => 4,
        BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => 5,
    }
}

fn binary_op(kind: TokenKind) -> Option<BinaryOp> {
    Some(match kind {
        TokenKind::OrOr => BinaryOp::Or,
        TokenKind::AndAnd => BinaryOp::And,
        TokenKind::EqEq => BinaryOp::Eq,
        TokenKind::BangEq => BinaryOp::NotEq,
        TokenKind::Lt => BinaryOp::Lt,
        TokenKind::LtEq => BinaryOp::LtEq,
        TokenKind::Gt => BinaryOp::Gt,
        TokenKind::GtEq => BinaryOp::GtEq,
        TokenKind::Plus => BinaryOp::Add,
        TokenKind::Minus => BinaryOp::Sub,
        TokenKind::Star => BinaryOp::Mul,
        TokenKind::Slash => BinaryOp::Div,
        TokenKind::Percent => BinaryOp::Rem,
        _ => return None,
    })
}

impl Parser<'_> {
    pub(super) fn parse_expr(&mut self, trailing: TrailingBlocks) -> ParseResult<Expr> {
        self.parse_binary(0, trailing)
    }

    fn parse_binary(&mut self, min_power: u8, trailing: TrailingBlocks) -> ParseResult<Expr> {
        let mut lhs = self.parse_prefix(trailing)?;
        while let Some(op) = binary_op(self.peek()) {
            let power = binding_power(op);
            if power <= min_power {
                break;
            }
            self.bump();
            self.skip_newlines();
            let rhs = self.parse_binary(power, trailing)?;
            if op.is_comparison() && binary_op(self.peek()).is_some_and(BinaryOp::is_comparison) {
                let chained = self.current().span;
                self.report(
                    "comparisons can't be chained; combine them with `&&`",
                    chained,
                );
            }
            lhs = Expr {
                span: lhs.span.to(rhs.span),
                kind: ExprKind::Binary {
                    op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
            };
        }
        Ok(lhs)
    }

    fn parse_prefix(&mut self, trailing: TrailingBlocks) -> ParseResult<Expr> {
        let op = match self.peek() {
            TokenKind::Bang => UnaryOp::Not,
            TokenKind::Minus => UnaryOp::Neg,
            TokenKind::Dollar => UnaryOp::Bind,
            _ => return self.parse_postfix(trailing),
        };
        let start = self.bump().span;
        let operand = self.parse_prefix(trailing)?;
        Ok(Expr {
            span: start.to(operand.span),
            kind: ExprKind::Unary {
                op,
                operand: Box::new(operand),
            },
        })
    }

    fn parse_postfix(&mut self, trailing: TrailingBlocks) -> ParseResult<Expr> {
        let mut expr = self.parse_primary()?;
        loop {
            expr = match self.peek() {
                TokenKind::LParen => self.parse_call(expr, trailing)?,
                TokenKind::Dot => {
                    self.bump();
                    let name = self.expect_ident()?;
                    Expr {
                        span: expr.span.to(name.span),
                        kind: ExprKind::Field {
                            base: Box::new(expr),
                            name,
                        },
                    }
                }
                TokenKind::LBracket => {
                    self.bump();
                    self.skip_newlines();
                    let index = self.parse_expr(TrailingBlocks::Allowed)?;
                    self.skip_newlines();
                    let close = self.expect(TokenKind::RBracket)?;
                    Expr {
                        span: expr.span.to(close.span),
                        kind: ExprKind::Index {
                            base: Box::new(expr),
                            index: Box::new(index),
                        },
                    }
                }
                _ => return Ok(expr),
            };
        }
    }

    pub(super) fn parse_call(
        &mut self,
        callee: Expr,
        trailing: TrailingBlocks,
    ) -> ParseResult<Expr> {
        self.bump();
        let args = self.comma_list(TokenKind::RParen, Self::parse_arg)?;
        self.check_argument_order(&args);
        let trailing_block = if trailing == TrailingBlocks::Allowed && self.at(TokenKind::LBrace) {
            Some(self.parse_block()?)
        } else {
            None
        };
        Ok(Expr {
            span: self.span_from(callee.span.start),
            kind: ExprKind::Call {
                callee: Box::new(callee),
                args,
                trailing_block,
            },
        })
    }

    fn parse_arg(&mut self) -> ParseResult<Arg> {
        let label = if self.at(TokenKind::Ident) && self.peek_nth(1) == TokenKind::Colon {
            let label = self.expect_ident()?;
            self.bump();
            self.skip_newlines();
            Some(label)
        } else {
            None
        };
        let value = self.parse_expr(TrailingBlocks::Allowed)?;
        Ok(Arg { label, value })
    }

    fn check_argument_order(&mut self, args: &[Arg]) {
        let mut named_seen = false;
        for arg in args {
            match arg.label {
                Some(_) => named_seen = true,
                None if named_seen => {
                    self.report(
                        "positional arguments must come before named ones",
                        arg.value.span,
                    );
                }
                None => {}
            }
        }
    }

    fn parse_primary(&mut self) -> ParseResult<Expr> {
        let token = self.current();
        let kind = match token.kind {
            TokenKind::Int => {
                self.bump();
                self.number_literal(
                    token.span,
                    ExprKind::Int,
                    "this number is too large for an `int`",
                )
            }
            TokenKind::Float => {
                self.bump();
                self.number_literal(token.span, ExprKind::Float, "this is not a valid `float`")
            }
            TokenKind::True | TokenKind::False => {
                self.bump();
                ExprKind::Bool(token.kind == TokenKind::True)
            }
            TokenKind::Ident => {
                self.bump();
                ExprKind::Name(self.text(token.span).to_owned())
            }
            TokenKind::StringStart => return self.parse_string(),
            TokenKind::Dot => {
                self.bump();
                ExprKind::ImplicitMember(self.expect_ident()?)
            }
            TokenKind::LBracket => {
                self.bump();
                let items = self.comma_list(TokenKind::RBracket, |parser| {
                    parser.parse_expr(TrailingBlocks::Allowed)
                })?;
                ExprKind::List(items)
            }
            TokenKind::LParen => {
                self.bump();
                self.skip_newlines();
                let inner = self.parse_expr(TrailingBlocks::Allowed)?;
                self.skip_newlines();
                self.expect(TokenKind::RParen)?;
                return Ok(inner);
            }
            _ => return self.unexpected("an expression"),
        };
        Ok(Expr {
            kind,
            span: self.span_from(token.span.start),
        })
    }

    fn parse_string(&mut self) -> ParseResult<Expr> {
        let start = self.bump().span.start;
        let mut parts = Vec::new();
        loop {
            match self.peek() {
                TokenKind::StringText => {
                    let text = self.bump();
                    parts.push(StringPart::Text(unescape(self.text(text.span))));
                }
                TokenKind::InterpolationStart => {
                    self.bump();
                    self.skip_newlines();
                    parts.push(StringPart::Expr(self.parse_expr(TrailingBlocks::Allowed)?));
                    self.skip_newlines();
                    self.expect(TokenKind::InterpolationEnd)?;
                }
                TokenKind::StringEnd => {
                    self.bump();
                    return Ok(Expr {
                        kind: ExprKind::String(parts),
                        span: self.span_from(start),
                    });
                }
                _ => return self.unexpected("the end of the string"),
            }
        }
    }

    /// Parses a number token, reporting `problem` if it doesn't fit the target type.
    fn number_literal<T: FromStr>(
        &mut self,
        span: Span,
        wrap: fn(T) -> ExprKind,
        problem: &str,
    ) -> ExprKind {
        if let Ok(value) = self.text(span).replace('_', "").parse() {
            wrap(value)
        } else {
            self.report(problem, span);
            ExprKind::Error
        }
    }
}

#[cfg(test)]
mod tests {
    use super::binary_op;
    use crate::token::SYMBOLS;

    #[test]
    fn binary_operators_round_trip_through_their_tokens() {
        for &kind in SYMBOLS {
            if let Some(op) = binary_op(kind) {
                assert_eq!(op.token(), kind);
            }
        }
    }
}
