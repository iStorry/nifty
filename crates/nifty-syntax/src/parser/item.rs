//! Top-level declarations and type expressions.

use super::{ParseResult, Parser};
use crate::ast::{
    FnDecl, Item, StateDecl, StructDecl, TypeExpr, TypeExprKind, TypedName, ViewDecl,
};
use crate::token::TokenKind;

use super::expr::TrailingBlocks;

impl Parser<'_> {
    pub(super) fn parse_item(&mut self) -> ParseResult<Item> {
        match self.peek() {
            TokenKind::State => self.parse_state().map(Item::State),
            TokenKind::Struct => self.parse_struct().map(Item::Struct),
            TokenKind::Fn => self.parse_fn().map(Item::Fn),
            TokenKind::View => self.parse_view().map(Item::View),
            _ => self.parse_stmt().map(Item::Stmt),
        }
    }

    fn parse_state(&mut self) -> ParseResult<StateDecl> {
        let start = self.bump().span.start;
        let name = self.expect_ident()?;
        let ty = self.parse_type_annotation()?;
        self.expect(TokenKind::Eq)?;
        let value = self.parse_expr(TrailingBlocks::Allowed)?;
        Ok(StateDecl {
            name,
            ty,
            value,
            span: self.span_from(start),
        })
    }

    fn parse_struct(&mut self) -> ParseResult<StructDecl> {
        let start = self.bump().span.start;
        let name = self.expect_ident()?;
        self.expect(TokenKind::LBrace)?;
        let mut fields = Vec::new();
        loop {
            self.skip_newlines();
            if self.eat(TokenKind::RBrace).is_some() {
                break;
            }
            fields.push(self.parse_typed_name()?);
            match self.peek() {
                TokenKind::Comma | TokenKind::Newline => {
                    self.bump();
                }
                TokenKind::RBrace => {}
                _ => return self.unexpected("`,` or a new line between fields"),
            }
        }
        Ok(StructDecl {
            name,
            fields,
            span: self.span_from(start),
        })
    }

    fn parse_fn(&mut self) -> ParseResult<FnDecl> {
        let start = self.bump().span.start;
        let name = self.expect_ident()?;
        self.expect(TokenKind::LParen)?;
        let params = self.comma_list(TokenKind::RParen, Self::parse_typed_name)?;
        let return_type = match self.eat(TokenKind::Arrow) {
            Some(_) => Some(self.parse_type()?),
            None => None,
        };
        let body = self.parse_block()?;
        Ok(FnDecl {
            name,
            params,
            return_type,
            body,
            span: self.span_from(start),
        })
    }

    fn parse_view(&mut self) -> ParseResult<ViewDecl> {
        let start = self.bump().span.start;
        let name = self.expect_ident()?;
        let params = match self.eat(TokenKind::LParen) {
            Some(_) => self.comma_list(TokenKind::RParen, Self::parse_typed_name)?,
            None => Vec::new(),
        };
        let body = self.parse_block()?;
        Ok(ViewDecl {
            name,
            params,
            body,
            span: self.span_from(start),
        })
    }

    fn parse_typed_name(&mut self) -> ParseResult<TypedName> {
        let name = self.expect_ident()?;
        self.expect(TokenKind::Colon)?;
        let ty = self.parse_type()?;
        Ok(TypedName { name, ty })
    }

    /// An optional `: Type`.
    pub(super) fn parse_type_annotation(&mut self) -> ParseResult<Option<TypeExpr>> {
        match self.eat(TokenKind::Colon) {
            Some(_) => self.parse_type().map(Some),
            None => Ok(None),
        }
    }

    fn parse_type(&mut self) -> ParseResult<TypeExpr> {
        let start = self.current().span.start;
        let kind = match self.peek() {
            TokenKind::Ident => {
                let token = self.bump();
                TypeExprKind::Named(self.text(token.span).to_owned())
            }
            TokenKind::LBracket => {
                self.bump();
                let element = self.parse_type()?;
                self.expect(TokenKind::RBracket)?;
                TypeExprKind::List(Box::new(element))
            }
            _ => return self.unexpected("a type"),
        };
        Ok(TypeExpr {
            kind,
            span: self.span_from(start),
        })
    }
}
