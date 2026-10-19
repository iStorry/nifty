//! Expressions and places.

use std::rc::Rc;

use nifty_syntax::Span;
use nifty_syntax::ast::{self, Arg, ExprKind as Ast, StringPart, UnaryOp};

use super::args::Args;
use super::{Checker, ELEMENTS, Global};
use crate::hir::{BinaryOp, Expr, ExprKind, Place, PlaceRoot, PlaceStep, Segment, StructId, Type};

impl Checker {
    /// Checks `expr`; `expected` guides literals that can't stand alone, such as `[]`.
    pub(super) fn expr(&mut self, expr: &ast::Expr, expected: Option<&Type>) -> Expr {
        let span = expr.span;
        let (kind, ty) = match &expr.kind {
            Ast::Int(value) => (ExprKind::Int(*value), Type::Int),
            Ast::Float(value) => (ExprKind::Float(*value), Type::Float),
            Ast::Bool(value) => (ExprKind::Bool(*value), Type::Bool),
            Ast::String(parts) => (self.string(parts), Type::String),
            Ast::Name(name) => return self.name(name, span),
            Ast::List(items) => return self.list(items, expected, span),
            Ast::Unary { op, operand } => return self.unary(*op, operand, span),
            Ast::Binary { op, lhs, rhs } => return self.binary(*op, lhs, rhs, span),
            Ast::Call {
                callee,
                args,
                trailing_block,
            } => {
                if let Some(block) = trailing_block {
                    self.error("only elements take a `{ … }` block", block.span);
                }
                return self.call(callee, args, span);
            }
            Ast::Field { base, name } => return self.field(base, name, span),
            Ast::Index { base, index } => return self.index(base, index, span),
            Ast::ImplicitMember(name) => {
                self.error(
                    format!(
                        "`.{}` only works where a style or alignment is expected",
                        name.name
                    ),
                    span,
                );
                (ExprKind::Error, Type::Error)
            }
            Ast::Error => (ExprKind::Error, Type::Error),
        };
        Expr { kind, ty, span }
    }

    /// Checks `expr` and reports it unless it has type `expected`.
    pub(super) fn expect(&mut self, expr: &ast::Expr, expected: &Type) -> Expr {
        let checked = self.expr(expr, Some(expected));
        self.require(&checked.ty, expected, checked.span);
        checked
    }

    fn string(&mut self, parts: &[StringPart]) -> ExprKind {
        if let [StringPart::Text(text)] = parts {
            return ExprKind::String(Rc::from(text.as_str()));
        }
        let segments = parts
            .iter()
            .map(|part| match part {
                StringPart::Text(text) => Segment::Text(Rc::from(text.as_str())),
                StringPart::Expr(value) => {
                    let value = self.expr(value, None);
                    if !matches!(
                        value.ty,
                        Type::Int | Type::Float | Type::Bool | Type::String | Type::Error
                    ) {
                        self.error(
                            format!("can't put a `{}` in a string", self.type_name(&value.ty)),
                            value.span,
                        );
                    }
                    Segment::Value(value)
                }
            })
            .collect();
        ExprKind::Interpolate(segments)
    }

    fn name(&mut self, name: &str, span: Span) -> Expr {
        if let Some(local) = self.body.lookup(name) {
            let ty = self.body.local(local).ty.clone();
            return Expr {
                kind: ExprKind::Local(local),
                ty,
                span,
            };
        }
        match self.globals.get(name).copied() {
            Some(Global::State(state)) => {
                self.effects.reads.insert(state);
                Expr {
                    kind: ExprKind::State(state),
                    ty: self.program.states[state.0].ty.clone(),
                    span,
                }
            }
            Some(Global::Struct(_)) => self.error_expr(
                format!("`{name}` is a struct; create one with `{name}(field: value)`"),
                span,
            ),
            Some(Global::Fn(_)) => self.error_expr(
                format!("`{name}` is a function; call it with `{name}(…)`"),
                span,
            ),
            Some(Global::View(_)) => self.error_expr(
                format!("`{name}` is a view; use it inside a window or view body"),
                span,
            ),
            None if ELEMENTS.contains(&name) => self.error_expr(
                format!("`{name}` is an element; use it inside a window or view body"),
                span,
            ),
            None => self.error_expr(format!("unknown name `{name}`"), span),
        }
    }

    fn list(&mut self, items: &[ast::Expr], expected: Option<&Type>, span: Span) -> Expr {
        let expected_element = match expected {
            Some(Type::List(element)) => Some(element.as_ref().clone()),
            _ => None,
        };
        let Some((first, rest)) = items.split_first() else {
            return match expected_element {
                Some(element) => Expr {
                    kind: ExprKind::List(Vec::new()),
                    ty: Type::List(Rc::new(element)),
                    span,
                },
                None => self.error_expr(
                    "can't tell what this empty list holds; add a type such as `: [int]`",
                    span,
                ),
            };
        };
        let first = self.expr(first, expected_element.as_ref());
        let element = expected_element.unwrap_or_else(|| first.ty.clone());
        self.require(&first.ty, &element, first.span);
        let mut checked = vec![first];
        checked.extend(rest.iter().map(|item| self.expect(item, &element)));
        Expr {
            kind: ExprKind::List(checked),
            ty: Type::List(Rc::new(element)),
            span,
        }
    }

    fn unary(&mut self, op: UnaryOp, operand: &ast::Expr, span: Span) -> Expr {
        match op {
            UnaryOp::Not => {
                let operand = self.expect(operand, &Type::Bool);
                Expr {
                    kind: ExprKind::Not(Box::new(operand)),
                    ty: Type::Bool,
                    span,
                }
            }
            UnaryOp::Neg => {
                let operand = self.expr(operand, None);
                if !matches!(operand.ty, Type::Int | Type::Float | Type::Error) {
                    self.error(
                        format!("can't negate a `{}`", self.type_name(&operand.ty)),
                        operand.span,
                    );
                }
                Expr {
                    ty: operand.ty.clone(),
                    kind: ExprKind::Negate(Box::new(operand)),
                    span,
                }
            }
            UnaryOp::Bind => self.error_expr(
                "`$` makes a two-way binding, which only `text_field` and `checkbox` accept",
                span,
            ),
        }
    }

    fn binary(&mut self, op: BinaryOp, lhs: &ast::Expr, rhs: &ast::Expr, span: Span) -> Expr {
        let lhs = self.expr(lhs, None);
        let rhs = self.expr(rhs, Some(&lhs.ty));
        let ty = self.binary_type(op, &lhs.ty, &rhs.ty, span);
        Expr {
            kind: ExprKind::Binary(op, Box::new(lhs), Box::new(rhs)),
            ty,
            span,
        }
    }

    pub(super) fn binary_type(&mut self, op: BinaryOp, lhs: &Type, rhs: &Type, span: Span) -> Type {
        if lhs.is_error() || rhs.is_error() {
            return Type::Error;
        }
        let result = match op {
            BinaryOp::And | BinaryOp::Or => {
                (*lhs == Type::Bool && *rhs == Type::Bool).then_some(Type::Bool)
            }
            BinaryOp::Eq | BinaryOp::NotEq => {
                (lhs == rhs && *lhs != Type::Void).then_some(Type::Bool)
            }
            BinaryOp::Lt | BinaryOp::LtEq | BinaryOp::Gt | BinaryOp::GtEq => (lhs == rhs
                && matches!(lhs, Type::Int | Type::Float | Type::String))
            .then_some(Type::Bool),
            BinaryOp::Add => (lhs == rhs && matches!(lhs, Type::Int | Type::Float | Type::String))
                .then(|| lhs.clone()),
            BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                (lhs == rhs && matches!(lhs, Type::Int | Type::Float)).then(|| lhs.clone())
            }
        };
        result.unwrap_or_else(|| {
            let hint = if op == BinaryOp::Add && (*lhs == Type::String || *rhs == Type::String) {
                "; to join text and a value, use interpolation: \"{value}\""
            } else {
                ""
            };
            self.error(
                format!(
                    "can't use `{}` with `{}` and `{}`{hint}",
                    op.symbol(),
                    self.type_name(lhs),
                    self.type_name(rhs)
                ),
                span,
            );
            Type::Error
        })
    }

    fn call(&mut self, callee: &ast::Expr, args: &[Arg], span: Span) -> Expr {
        match &callee.kind {
            Ast::Field { base, name } => self.method(base, name, args, span),
            Ast::Name(name) if self.body.lookup(name).is_none() => {
                match self.globals.get(name).copied() {
                    Some(Global::Fn(function)) => self.call_function(function, name, args, span),
                    Some(Global::Struct(id)) => self.construct(id, args, span),
                    Some(Global::View(_)) => self.error_expr(
                        format!("`{name}` is a view; use it inside a window or view body"),
                        span,
                    ),
                    _ if ELEMENTS.contains(&name.as_str()) => self.error_expr(
                        format!("`{name}` is an element; use it inside a window or view body"),
                        span,
                    ),
                    _ => self.error_expr(format!("unknown function `{name}`"), callee.span),
                }
            }
            _ => self.error_expr("this can't be called", callee.span),
        }
    }

    fn call_function(
        &mut self,
        function: crate::hir::FnId,
        name: &str,
        args: &[Arg],
        span: Span,
    ) -> Expr {
        let signature = &self.fn_signatures[function.0];
        let params = signature.params.clone();
        let return_type = signature.return_type.clone();
        let mut args = Args::new(self, format!("`{name}`"), args);
        let mut checked = Vec::new();
        for (param, ty) in &params {
            let arg = args.param(param);
            checked.push(self.required_arg(arg, ty, span, || {
                format!("`{name}` needs an argument for `{param}`")
            }));
        }
        args.finish(self);
        self.effects.calls.insert(function);
        Expr {
            kind: ExprKind::Call(function, checked),
            ty: return_type,
            span,
        }
    }

    fn construct(&mut self, id: StructId, args: &[Arg], span: Span) -> Expr {
        let name = self.program.structs[id.0].name.clone();
        let fields: Vec<(String, Type)> = self.program.structs[id.0]
            .fields
            .iter()
            .map(|field| (field.name.clone(), field.ty.clone()))
            .collect();
        let mut args = Args::new(self, format!("`{name}`"), args);
        let mut checked = Vec::new();
        for (field, ty) in &fields {
            let arg = args.named(field);
            checked.push(self.required_arg(arg, ty, span, || {
                format!("`{name}` needs a value for `{field}`")
            }));
        }
        args.finish(self);
        Expr {
            kind: ExprKind::Construct(id, checked),
            ty: Type::Struct(id),
            span,
        }
    }

    fn method(&mut self, base: &ast::Expr, name: &ast::Ident, args: &[Arg], span: Span) -> Expr {
        match name.name.as_str() {
            "len" | "is_empty" => {
                let receiver = self.expr(base, None);
                self.no_args(&name.name, args);
                if !matches!(receiver.ty, Type::List(_) | Type::String | Type::Error) {
                    return self.no_method(&receiver.ty, name);
                }
                let receiver = Box::new(receiver);
                let (kind, ty) = if name.name == "len" {
                    (ExprKind::Len(receiver), Type::Int)
                } else {
                    (ExprKind::IsEmpty(receiver), Type::Bool)
                };
                Expr { kind, ty, span }
            }
            "push" | "remove" => {
                let Some(place) = self.place(base) else {
                    return self
                        .error_expr("only a list in state or a variable can change", base.span);
                };
                let Type::List(element) = place.ty.clone() else {
                    if place.ty.is_error() {
                        return reported(span);
                    }
                    return self.no_method(&place.ty, name);
                };
                self.note_write(&place);
                let [arg] = args else {
                    return self.error_expr(format!("`{}` takes one argument", name.name), span);
                };
                if name.name == "push" {
                    let value = self.expect(&arg.value, &element);
                    Expr {
                        kind: ExprKind::Push(place, Box::new(value)),
                        ty: Type::Void,
                        span,
                    }
                } else {
                    let index = self.expect(&arg.value, &Type::Int);
                    Expr {
                        kind: ExprKind::Remove(place, Box::new(index)),
                        ty: Type::Void,
                        span,
                    }
                }
            }
            _ => {
                let receiver = self.expr(base, None);
                if receiver.ty.is_error() {
                    return reported(span);
                }
                self.no_method(&receiver.ty, name)
            }
        }
    }

    fn no_args(&mut self, method: &str, args: &[Arg]) {
        if let Some(first) = args.first() {
            self.error(format!("`{method}` takes no arguments"), first.value.span);
        }
    }

    fn no_method(&mut self, ty: &Type, name: &ast::Ident) -> Expr {
        let available = match ty {
            Type::List(_) => "; lists have `len`, `is_empty`, `push` and `remove`",
            Type::String => "; strings have `len` and `is_empty`",
            _ => "",
        };
        self.error_expr(
            format!(
                "`{}` has no method `{}`{available}",
                self.type_name(ty),
                name.name
            ),
            name.span,
        )
    }

    fn field(&mut self, base: &ast::Expr, name: &ast::Ident, span: Span) -> Expr {
        let base = self.expr(base, None);
        match self.field_index(&base.ty, name) {
            Some((index, ty)) => Expr {
                kind: ExprKind::Field(Box::new(base), index),
                ty,
                span,
            },
            None => reported(span),
        }
    }

    /// The field's position and type, or `None` after reporting why there is no such field.
    fn field_index(&mut self, ty: &Type, name: &ast::Ident) -> Option<(usize, Type)> {
        match ty {
            Type::Struct(id) => {
                let found = self.program.structs[id.0]
                    .fields
                    .iter()
                    .enumerate()
                    .find(|(_, field)| field.name == name.name)
                    .map(|(index, field)| (index, field.ty.clone()));
                if found.is_none() {
                    self.error(
                        format!("`{}` has no field `{}`", self.type_name(ty), name.name),
                        name.span,
                    );
                }
                found
            }
            Type::Error => None,
            _ => {
                self.error(
                    format!(
                        "`{}` has no fields; `{}` isn't one",
                        self.type_name(ty),
                        name.name
                    ),
                    name.span,
                );
                None
            }
        }
    }

    fn index(&mut self, base: &ast::Expr, index: &ast::Expr, span: Span) -> Expr {
        let base = self.expr(base, None);
        let index = self.expect(index, &Type::Int);
        let ty = match &base.ty {
            Type::List(element) => element.as_ref().clone(),
            Type::Error => Type::Error,
            other => {
                self.error(
                    format!("can't index into a `{}`", self.type_name(other)),
                    base.span,
                );
                Type::Error
            }
        };
        Expr {
            kind: ExprKind::Index(Box::new(base), Box::new(index)),
            ty,
            span,
        }
    }

    /// Converts an assignable expression into a place, or reports why it can't be changed.
    pub(super) fn place(&mut self, expr: &ast::Expr) -> Option<Place> {
        let span = expr.span;
        match &expr.kind {
            Ast::Name(name) => {
                if let Some(local) = self.body.lookup(name) {
                    let ty = self.body.local(local).ty.clone();
                    return Some(Place {
                        root: PlaceRoot::Local(local),
                        path: Vec::new(),
                        ty,
                        span,
                    });
                }
                if let Some(Global::State(state)) = self.globals.get(name).copied() {
                    return Some(Place {
                        root: PlaceRoot::State(state),
                        path: Vec::new(),
                        ty: self.program.states[state.0].ty.clone(),
                        span,
                    });
                }
                self.name(name, span);
                None
            }
            Ast::Field { base, name } => {
                let mut place = self.place(base)?;
                let (index, ty) = self.field_index(&place.ty, name)?;
                place.path.push(PlaceStep::Field(index));
                place.ty = ty;
                place.span = span;
                Some(place)
            }
            Ast::Index { base, index } => {
                let mut place = self.place(base)?;
                let index = self.expect(index, &Type::Int);
                place.ty = match &place.ty {
                    Type::List(element) => element.as_ref().clone(),
                    other => {
                        self.error(
                            format!("can't index into a `{}`", self.type_name(other)),
                            place.span,
                        );
                        return None;
                    }
                };
                place.path.push(PlaceStep::Index(index));
                place.span = span;
                Some(place)
            }
            _ => {
                self.error(
                    "this can't be changed; only state, variables, fields and list items can",
                    span,
                );
                None
            }
        }
    }

    /// Whether `place` changes state: directly, or through a `for` variable aliasing it.
    pub(super) fn writes_state(&self, place: &Place) -> bool {
        match place.root {
            PlaceRoot::State(_) => true,
            PlaceRoot::Local(local) => self.body.local(local).alias,
        }
    }

    pub(super) fn note_write(&mut self, place: &Place) {
        if self.writes_state(place) {
            self.effects.writes = true;
        }
        if let PlaceRoot::State(state) = place.root {
            self.effects.reads.insert(state);
        }
    }

    pub(super) fn error_expr(&mut self, message: impl Into<String>, span: Span) -> Expr {
        self.error(message, span);
        reported(span)
    }

    /// Checks a required argument, or reports it missing and stands in an error expression.
    pub(super) fn required_arg(
        &mut self,
        arg: Option<&ast::Expr>,
        ty: &Type,
        span: Span,
        missing: impl FnOnce() -> String,
    ) -> Expr {
        if let Some(arg) = arg {
            return self.expect(arg, ty);
        }
        self.error(missing(), span);
        reported(span)
    }
}

/// An error expression for a problem that was already reported.
pub(super) fn reported(span: Span) -> Expr {
    Expr {
        kind: ExprKind::Error,
        ty: Type::Error,
        span,
    }
}
