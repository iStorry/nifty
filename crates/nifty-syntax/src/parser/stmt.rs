//! Statements and blocks.

use super::expr::TrailingBlocks;
use super::{ParseResult, Parser, Reported};
use crate::ast::{AssignOp, Block, ElseBranch, IfStmt, Iterable, Stmt, StmtKind};
use crate::token::TokenKind;

impl Parser<'_> {
    pub(super) fn parse_block(&mut self) -> ParseResult<Block> {
        let start = self.expect(TokenKind::LBrace)?.span.start;
        let mut stmts = Vec::new();
        loop {
            self.skip_newlines();
            match self.peek() {
                TokenKind::RBrace => {
                    self.bump();
                    break;
                }
                TokenKind::Eof => return self.unexpected("`}`"),
                _ => match self.parse_stmt() {
                    Ok(stmt) => {
                        stmts.push(stmt);
                        self.finish_line();
                    }
                    Err(Reported) => self.recover_to_line_end(),
                },
            }
        }
        Ok(Block {
            stmts,
            span: self.span_from(start),
        })
    }

    pub(super) fn parse_stmt(&mut self) -> ParseResult<Stmt> {
        let start = self.current().span.start;
        let kind = match self.peek() {
            TokenKind::Let => self.parse_let()?,
            TokenKind::If => StmtKind::If(self.parse_if()?),
            TokenKind::For => self.parse_for()?,
            TokenKind::Return => self.parse_return()?,
            _ => self.parse_expr_or_assign()?,
        };
        Ok(Stmt {
            kind,
            span: self.span_from(start),
        })
    }

    fn parse_let(&mut self) -> ParseResult<StmtKind> {
        self.bump();
        let name = self.expect_ident()?;
        let ty = self.parse_type_annotation()?;
        self.expect(TokenKind::Eq)?;
        let value = self.parse_expr(TrailingBlocks::Allowed)?;
        Ok(StmtKind::Let { name, ty, value })
    }

    fn parse_if(&mut self) -> ParseResult<IfStmt> {
        self.bump();
        let condition = self.parse_expr(TrailingBlocks::Forbidden)?;
        let then_block = self.parse_block()?;
        let else_branch = match self.eat(TokenKind::Else) {
            None => None,
            Some(_) if self.at(TokenKind::If) => Some(ElseBranch::If(Box::new(self.parse_if()?))),
            Some(_) => Some(ElseBranch::Block(self.parse_block()?)),
        };
        Ok(IfStmt {
            condition,
            then_block,
            else_branch,
        })
    }

    fn parse_for(&mut self) -> ParseResult<StmtKind> {
        self.bump();
        let first = self.expect_ident()?;
        let (mut index, item) = match self.eat(TokenKind::Comma) {
            Some(_) => (Some(first), self.expect_ident()?),
            None => (None, first),
        };
        self.expect(TokenKind::In)?;
        let iterable = self.parse_iterable()?;
        if let (Some(extra), Iterable::Range { .. }) = (&index, &iterable) {
            self.report(
                format!(
                    "a range loop has no separate index; `{}` already counts, as in `for {} in 0..n`",
                    item.name, item.name
                ),
                extra.span,
            );
            index = None;
        }
        let body = self.parse_block()?;
        Ok(StmtKind::For {
            index,
            item,
            iterable,
            body,
        })
    }

    fn parse_iterable(&mut self) -> ParseResult<Iterable> {
        let start = self.parse_expr(TrailingBlocks::Forbidden)?;
        if self.eat(TokenKind::DotDot).is_none() {
            return Ok(Iterable::List(start));
        }
        let end = self.parse_expr(TrailingBlocks::Forbidden)?;
        Ok(Iterable::Range { start, end })
    }

    fn parse_return(&mut self) -> ParseResult<StmtKind> {
        self.bump();
        let value = match self.peek() {
            TokenKind::Newline | TokenKind::RBrace | TokenKind::Eof => None,
            _ => Some(self.parse_expr(TrailingBlocks::Allowed)?),
        };
        Ok(StmtKind::Return(value))
    }

    fn parse_expr_or_assign(&mut self) -> ParseResult<StmtKind> {
        let target = self.parse_expr(TrailingBlocks::Allowed)?;
        let Some(op) = assign_op(self.peek()) else {
            return Ok(StmtKind::Expr(target));
        };
        self.bump();
        self.skip_newlines();
        let value = self.parse_expr(TrailingBlocks::Allowed)?;
        Ok(StmtKind::Assign { target, op, value })
    }
}

fn assign_op(kind: TokenKind) -> Option<AssignOp> {
    Some(match kind {
        TokenKind::Eq => AssignOp::Set,
        TokenKind::PlusEq => AssignOp::Add,
        TokenKind::MinusEq => AssignOp::Sub,
        TokenKind::StarEq => AssignOp::Mul,
        TokenKind::SlashEq => AssignOp::Div,
        _ => return None,
    })
}
