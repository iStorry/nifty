//! Statements in function bodies and button actions.

use nifty_syntax::Span;
use nifty_syntax::ast::{self, AssignOp, ElseBranch, ExprKind as Ast, Ident, IfStmt, StmtKind};

use super::Checker;
use super::env::{Context, LocalInfo, LocalKind};
use super::expr::reported;
use crate::hir::{BinaryOp, Block, ExprKind, Iterable, Loop, PlaceRoot, Stmt, Type};

impl Checker {
    pub(super) fn block(&mut self, block: &ast::Block) -> Block {
        self.body.push_scope();
        let stmts: Vec<Stmt> = block.stmts.iter().filter_map(|s| self.stmt(s)).collect();
        self.body.pop_scope();
        stmts.into()
    }

    fn stmt(&mut self, stmt: &ast::Stmt) -> Option<Stmt> {
        match &stmt.kind {
            StmtKind::Let { name, ty, value } => {
                let annotation = ty.as_ref().map(|ty| self.resolve_type(ty));
                let value = self.expr(value, annotation.as_ref());
                let ty = match annotation {
                    Some(annotation) => {
                        self.require(&value.ty, &annotation, value.span);
                        annotation
                    }
                    None => value.ty.clone(),
                };
                if ty == Type::Void {
                    self.error("this has no value to store", value.span);
                }
                let local = self.body.declare(
                    &name.name,
                    LocalInfo {
                        ty,
                        kind: LocalKind::Variable,
                    },
                );
                Some(Stmt::Let(local, value))
            }
            StmtKind::Assign { target, op, value } => self.assign(target, *op, value),
            StmtKind::If(if_stmt) => Some(self.if_stmt(if_stmt)),
            StmtKind::For {
                index,
                item,
                iterable,
                body,
            } => {
                let head = self.loop_head(iterable, index.is_some());
                let body = self.in_loop(index.as_ref(), item, &head, |checker| checker.block(body));
                Some(Stmt::For { head, body })
            }
            StmtKind::Return(value) => Some(self.return_stmt(value.as_ref(), stmt.span)),
            StmtKind::Expr(expr) => {
                let checked = self.expr(expr, None);
                let is_call = matches!(
                    checked.kind,
                    ExprKind::Call(..)
                        | ExprKind::Push(..)
                        | ExprKind::Remove(..)
                        | ExprKind::Download(..)
                        | ExprKind::Dialog(..)
                        | ExprKind::Error
                );
                if !is_call {
                    self.error("this value isn't used", expr.span);
                }
                Some(Stmt::Expr(checked))
            }
        }
    }

    fn assign(&mut self, target: &ast::Expr, op: AssignOp, value: &ast::Expr) -> Option<Stmt> {
        let place = self.place(target)?;
        self.note_write(&place);
        let op = match op {
            AssignOp::Set => None,
            AssignOp::Add => Some(BinaryOp::Add),
            AssignOp::Sub => Some(BinaryOp::Sub),
            AssignOp::Mul => Some(BinaryOp::Mul),
            AssignOp::Div => Some(BinaryOp::Div),
        };
        let value = self.expect(value, &place.ty);
        // A mismatched value was just reported; checking the operator too would repeat it.
        if let Some(op) = op
            && value.ty == place.ty
        {
            self.binary_type(op, &place.ty, &value.ty, value.span);
        }
        Some(Stmt::Assign { place, op, value })
    }

    fn if_stmt(&mut self, if_stmt: &IfStmt) -> Stmt {
        let mut branches = Vec::new();
        let mut current = if_stmt;
        loop {
            let condition = self.expect(&current.condition, &Type::Bool);
            branches.push((condition, self.block(&current.then_block)));
            match &current.else_branch {
                None => {
                    return Stmt::If {
                        branches,
                        otherwise: None,
                    };
                }
                Some(ElseBranch::Block(block)) => {
                    return Stmt::If {
                        branches,
                        otherwise: Some(self.block(block)),
                    };
                }
                Some(ElseBranch::If(next)) => current = next,
            }
        }
    }

    /// Checks what a `for` loops over and allocates its variables. The item aliases each
    /// element when the list is a place. [`Self::in_loop`] names the variables.
    pub(super) fn loop_head(&mut self, iterable: &ast::Iterable, has_index: bool) -> Loop {
        let (iterable, item) = match iterable {
            ast::Iterable::List(list) => self.list_head(list),
            ast::Iterable::Range { start, end } => {
                let start = self.expect(start, &Type::Int);
                let end = self.expect(end, &Type::Int);
                (Iterable::Range { start, end }, counter())
            }
        };
        Loop {
            item: self.body.allocate(item),
            index: has_index.then(|| self.body.allocate(counter())),
            iterable,
        }
    }

    fn list_head(&mut self, list: &ast::Expr) -> (Iterable, LocalInfo) {
        let (iterable, list_type, kind) = if is_place_expr(list) {
            match self.place(list) {
                Some(place) => {
                    if let PlaceRoot::State(state) = place.root {
                        self.effects.reads.insert(state);
                    }
                    let ty = place.ty.clone();
                    (Iterable::Place(place), ty, LocalKind::Alias)
                }
                None => (
                    Iterable::Value(reported(list.span)),
                    Type::Error,
                    LocalKind::Variable,
                ),
            }
        } else {
            let value = self.expr(list, None);
            let ty = value.ty.clone();
            (Iterable::Value(value), ty, LocalKind::Variable)
        };
        let ty = match &list_type {
            Type::List(element) => element.as_ref().clone(),
            Type::Error => Type::Error,
            other => {
                self.error(
                    format!(
                        "can only loop over a list or a range such as `0..n`, not a `{}`",
                        self.type_name(other)
                    ),
                    list.span,
                );
                Type::Error
            }
        };
        (iterable, LocalInfo { ty, kind })
    }

    /// Runs `check` on a loop's body with the loop's variables named.
    pub(super) fn in_loop<T>(
        &mut self,
        index: Option<&Ident>,
        item: &Ident,
        head: &Loop,
        check: impl FnOnce(&mut Self) -> T,
    ) -> T {
        self.body.push_scope();
        if let (Some(index), Some(local)) = (index, head.index) {
            if index.name == item.name {
                self.error(
                    format!("`{}` names both the index and the item", item.name),
                    item.span,
                );
            }
            self.body.bind(&index.name, local);
        }
        self.body.bind(&item.name, head.item);
        let checked = check(self);
        self.body.pop_scope();
        checked
    }

    fn return_stmt(&mut self, value: Option<&ast::Expr>, span: Span) -> Stmt {
        let Context::Function { return_type } = self.body.context.clone() else {
            self.error("`return` only works inside a function", span);
            return Stmt::Return(None);
        };
        match (value, &return_type) {
            (None, Type::Void) => Stmt::Return(None),
            (None, expected) => {
                self.error(
                    format!("this function must return a `{}`", self.type_name(expected)),
                    span,
                );
                Stmt::Return(None)
            }
            (Some(value), Type::Void) => {
                let value = self.expr(value, None);
                self.error("this function doesn't return a value", value.span);
                Stmt::Return(Some(value))
            }
            (Some(value), expected) => Stmt::Return(Some(self.expect(value, expected))),
        }
    }
}

/// Whether the expression names a location (`todos`, `a.b`, `xs[0]`) rather than computing
/// a new value.
fn is_place_expr(expr: &ast::Expr) -> bool {
    match &expr.kind {
        Ast::Name(_) => true,
        Ast::Field { base, .. } | Ast::Index { base, .. } => is_place_expr(base),
        _ => false,
    }
}

/// A `for` loop's index or range counter.
fn counter() -> LocalInfo {
    LocalInfo {
        ty: Type::Int,
        kind: LocalKind::Counter,
    }
}

/// Whether every path through `block` ends in `return`.
pub(super) fn always_returns(block: &[Stmt]) -> bool {
    match block.last() {
        Some(Stmt::Return(_)) => true,
        Some(Stmt::If {
            branches,
            otherwise: Some(otherwise),
        }) => branches.iter().all(|(_, body)| always_returns(body)) && always_returns(otherwise),
        _ => false,
    }
}
