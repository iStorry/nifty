//! Builds an [`Index`] in two passes. Top-level declarations may appear in any order and in
//! any file, so the first pass declares them all and the second resolves the names used in
//! them.

use std::collections::HashMap;

use nifty_syntax::Span;
use nifty_syntax::ast::{
    Arg, Block, ElseBranch, Expr, ExprKind, File, Ident, IfStmt, Item, Iterable, StmtKind,
    StringPart, TypeExpr, TypeExprKind, TypedName,
};

use super::{Index, Occurrence, Role, Symbol, SymbolId, SymbolKind, Unresolved, Visibility};

pub(super) fn build(files: &[&File]) -> Index {
    let mut builder = Builder {
        symbols: Vec::new(),
        occurrences: Vec::new(),
        scopes: vec![HashMap::new()],
        blocks: Vec::new(),
    };
    let items = || files.iter().flat_map(|file| &file.items);
    let declared: Vec<Declared> = items().map(|item| builder.declare_item(item)).collect();
    for (item, declared) in items().zip(declared) {
        builder.resolve_item(item, &declared);
    }
    builder
        .occurrences
        .sort_by_key(|occurrence| occurrence.span);
    Index {
        symbols: builder.symbols,
        occurrences: builder.occurrences,
    }
}

/// The parameters declared for a function or view in the first pass, bound in the second.
#[derive(Default)]
struct Declared {
    params: Vec<SymbolId>,
}

/// How a bare name is used, which decides what an unresolved one is reported as.
#[derive(Clone, Copy)]
enum NameUse {
    Value,
    Callee,
}

struct Builder<'ast> {
    symbols: Vec<Symbol>,
    occurrences: Vec<Occurrence>,
    /// Innermost last. The first scope holds the top-level declarations.
    scopes: Vec<HashMap<&'ast str, SymbolId>>,
    /// The blocks enclosing the statement being resolved, innermost last.
    blocks: Vec<Span>,
}

impl<'ast> Builder<'ast> {
    fn declare_item(&mut self, item: &'ast Item) -> Declared {
        match item {
            Item::State(decl) => {
                self.declare_global(&decl.name, SymbolKind::State);
                Declared::default()
            }
            Item::Struct(decl) => {
                let id = self.declare_global(&decl.name, SymbolKind::Struct);
                for field in &decl.fields {
                    self.declare(
                        &field.name,
                        SymbolKind::Field,
                        Some(id),
                        Visibility::Everywhere,
                    );
                }
                Declared::default()
            }
            Item::Fn(decl) => {
                self.declare_callable(&decl.name, SymbolKind::Function, &decl.params, &decl.body)
            }
            Item::View(decl) => {
                self.declare_callable(&decl.name, SymbolKind::View, &decl.params, &decl.body)
            }
            Item::Named(named) => {
                self.declare_global(&named.name, SymbolKind::Window);
                Declared::default()
            }
            Item::Stmt(_) => Declared::default(),
        }
    }

    fn declare_callable(
        &mut self,
        name: &'ast Ident,
        kind: SymbolKind,
        params: &[TypedName],
        body: &Block,
    ) -> Declared {
        let id = self.declare_global(name, kind);
        let visibility = Visibility::Within(body.span);
        let params = params
            .iter()
            .map(|param| self.declare(&param.name, SymbolKind::Parameter, Some(id), visibility))
            .collect();
        Declared { params }
    }

    fn resolve_item(&mut self, item: &'ast Item, declared: &Declared) {
        match item {
            Item::State(decl) => {
                self.resolve_type_annotation(decl.ty.as_ref());
                self.resolve_expr(&decl.value);
            }
            Item::Struct(decl) => {
                for field in &decl.fields {
                    self.resolve_type(&field.ty);
                }
            }
            Item::Fn(decl) => {
                self.resolve_type_annotation(decl.return_type.as_ref());
                self.resolve_callable_body(&decl.params, &declared.params, &decl.body);
            }
            Item::View(decl) => {
                self.resolve_callable_body(&decl.params, &declared.params, &decl.body);
            }
            Item::Named(named) => self.resolve_stmt(&named.call.kind),
            Item::Stmt(stmt) => self.resolve_stmt(&stmt.kind),
        }
    }

    fn resolve_callable_body(
        &mut self,
        params: &'ast [TypedName],
        ids: &[SymbolId],
        body: &'ast Block,
    ) {
        self.scopes.push(HashMap::new());
        for (param, &id) in params.iter().zip(ids) {
            self.resolve_type(&param.ty);
            self.bind(&param.name.name, id);
        }
        self.resolve_block(body);
        self.scopes.pop();
    }

    fn resolve_block(&mut self, block: &'ast Block) {
        self.scopes.push(HashMap::new());
        self.blocks.push(block.span);
        for stmt in &block.stmts {
            self.resolve_stmt(&stmt.kind);
        }
        self.blocks.pop();
        self.scopes.pop();
    }

    fn resolve_stmt(&mut self, stmt: &'ast StmtKind) {
        match stmt {
            StmtKind::Let { name, ty, value } => {
                self.resolve_type_annotation(ty.as_ref());
                self.resolve_expr(value);
                let visibility = Visibility::After {
                    file: value.span.file,
                    start: value.span.end,
                    end: self.blocks.last().map(|block| block.end),
                };
                let id = self.declare(name, SymbolKind::Local, None, visibility);
                self.bind(&name.name, id);
            }
            StmtKind::Assign { target, value, .. } => {
                self.resolve_expr(target);
                self.resolve_expr(value);
            }
            StmtKind::If(if_stmt) => self.resolve_if(if_stmt),
            StmtKind::For {
                index,
                item,
                iterable,
                body,
            } => {
                match iterable {
                    Iterable::List(list) => self.resolve_expr(list),
                    Iterable::Range { start, end } => {
                        self.resolve_expr(start);
                        self.resolve_expr(end);
                    }
                }
                self.scopes.push(HashMap::new());
                let visibility = Visibility::Within(body.span);
                for name in index.iter().chain([item]) {
                    let id = self.declare(name, SymbolKind::Local, None, visibility);
                    self.bind(&name.name, id);
                }
                self.resolve_block(body);
                self.scopes.pop();
            }
            StmtKind::Return(value) => {
                if let Some(value) = value {
                    self.resolve_expr(value);
                }
            }
            StmtKind::Expr(expr) => self.resolve_expr(expr),
        }
    }

    fn resolve_if(&mut self, if_stmt: &'ast IfStmt) {
        self.resolve_expr(&if_stmt.condition);
        self.resolve_block(&if_stmt.then_block);
        match &if_stmt.else_branch {
            Some(ElseBranch::If(nested)) => self.resolve_if(nested),
            Some(ElseBranch::Block(block)) => self.resolve_block(block),
            None => {}
        }
    }

    fn resolve_expr(&mut self, expr: &'ast Expr) {
        match &expr.kind {
            ExprKind::Name(name) => {
                self.resolve_name(name, expr.span, NameUse::Value);
            }
            ExprKind::ImplicitMember(member) => {
                self.unresolved(member.span, Unresolved::ImplicitMember);
            }
            ExprKind::String(parts) => {
                for part in parts {
                    if let StringPart::Expr(expr) = part {
                        self.resolve_expr(expr);
                    }
                }
            }
            ExprKind::List(items) => {
                for item in items {
                    self.resolve_expr(item);
                }
            }
            ExprKind::Unary { operand, .. } => self.resolve_expr(operand),
            ExprKind::Binary { lhs, rhs, .. } => {
                self.resolve_expr(lhs);
                self.resolve_expr(rhs);
            }
            ExprKind::Call {
                callee,
                args,
                trailing_block,
            } => {
                let callee = self.resolve_callee(callee);
                self.resolve_args(callee, args);
                if let Some(block) = trailing_block {
                    self.resolve_block(block);
                }
            }
            ExprKind::Field { base, name } => {
                self.resolve_expr(base);
                self.unresolved(name.span, Unresolved::Field);
            }
            ExprKind::Index { base, index } => {
                self.resolve_expr(base);
                self.resolve_expr(index);
            }
            ExprKind::Int(_) | ExprKind::Float(_) | ExprKind::Bool(_) | ExprKind::Error => {}
        }
    }

    /// Resolves what is being called, returning its symbol when it is declared in the file.
    fn resolve_callee(&mut self, callee: &'ast Expr) -> Option<SymbolId> {
        match &callee.kind {
            ExprKind::Name(name) => self.resolve_name(name, callee.span, NameUse::Callee),
            ExprKind::Field { base, name } => {
                self.resolve_expr(base);
                self.unresolved(name.span, Unresolved::Method);
                None
            }
            _ => {
                self.resolve_expr(callee);
                None
            }
        }
    }

    /// Argument labels of a call to a declared struct, function or view refer to its fields
    /// or parameters.
    fn resolve_args(&mut self, callee: Option<SymbolId>, args: &'ast [Arg]) {
        for arg in args {
            let member = callee
                .zip(arg.label.as_ref())
                .and_then(|(callee, label)| Some((self.member(callee, &label.name)?, label.span)));
            if let Some((member, span)) = member {
                self.occur(span, Role::Reference(member));
            }
            self.resolve_expr(&arg.value);
        }
    }

    fn resolve_name(&mut self, name: &str, span: Span, name_use: NameUse) -> Option<SymbolId> {
        let found = self.lookup(name);
        match found {
            Some(id) => self.occur(span, Role::Reference(id)),
            None => self.unresolved(
                span,
                match name_use {
                    NameUse::Value => Unresolved::Variable,
                    NameUse::Callee => Unresolved::Function,
                },
            ),
        }
        found
    }

    fn resolve_type_annotation(&mut self, ty: Option<&TypeExpr>) {
        if let Some(ty) = ty {
            self.resolve_type(ty);
        }
    }

    fn resolve_type(&mut self, ty: &TypeExpr) {
        match &ty.kind {
            TypeExprKind::Named(name) => {
                let declared_struct = self.scopes[0]
                    .get(name.as_str())
                    .copied()
                    .filter(|&id| self.symbols[id.0].kind == SymbolKind::Struct);
                match declared_struct {
                    Some(id) => self.occur(ty.span, Role::Reference(id)),
                    None => self.unresolved(ty.span, Unresolved::Type),
                }
            }
            TypeExprKind::List(element) => self.resolve_type(element),
        }
    }

    fn lookup(&self, name: &str) -> Option<SymbolId> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }

    fn member(&self, parent: SymbolId, name: &str) -> Option<SymbolId> {
        self.symbols
            .iter()
            .position(|symbol| symbol.parent == Some(parent) && symbol.name == name)
            .map(SymbolId)
    }

    /// Declares a top-level name. When a name is declared twice, the first one wins.
    fn declare_global(&mut self, name: &'ast Ident, kind: SymbolKind) -> SymbolId {
        let id = self.declare(name, kind, None, Visibility::Everywhere);
        self.scopes[0].entry(&name.name).or_insert(id);
        id
    }

    fn declare(
        &mut self,
        name: &Ident,
        kind: SymbolKind,
        parent: Option<SymbolId>,
        visibility: Visibility,
    ) -> SymbolId {
        let id = SymbolId(self.symbols.len());
        self.symbols.push(Symbol {
            name: name.name.clone(),
            kind,
            parent,
            visibility,
        });
        self.occur(name.span, Role::Declaration(id));
        id
    }

    fn bind(&mut self, name: &'ast str, id: SymbolId) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, id);
        }
    }

    fn unresolved(&mut self, span: Span, kind: Unresolved) {
        self.occur(span, Role::Unresolved(kind));
    }

    fn occur(&mut self, span: Span, role: Role) {
        self.occurrences.push(Occurrence { span, role });
    }
}
