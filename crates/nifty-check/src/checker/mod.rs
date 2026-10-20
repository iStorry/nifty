//! Checks a parsed file in phases: declarations, state, functions, then the UI. Functions are
//! checked before the UI so the UI knows what state every function call reads.

mod args;
mod builtins;
mod consts;
mod env;
mod expr;
mod stmt;
mod ui;

use std::collections::HashMap;
use std::mem;
use std::rc::Rc;

use nifty_syntax::ast::{self, Item, TypeExpr, TypeExprKind};
use nifty_syntax::{Diagnostic, Span};

use crate::hir::{
    FieldDef, FnDef, FnId, Program, StateDef, StateId, StructDef, StructId, Type, ViewDef, ViewId,
};
use crate::{elements, functions};
use env::{Body, Context, Effects, FnEffects, LocalInfo};

#[derive(Clone, Copy, Debug)]
enum Global {
    State(StateId),
    Struct(StructId),
    Fn(FnId),
    View(ViewId),
}

struct Signature {
    params: Vec<(String, Type)>,
    return_type: Type,
}

pub(crate) struct Checker {
    diagnostics: Vec<Diagnostic>,
    globals: HashMap<String, Global>,
    program: Program,
    fn_signatures: Vec<Signature>,
    view_params: Vec<Vec<(String, Type)>>,
    fn_effects: FnEffects,
    body: Body,
    effects: Effects,
}

pub(crate) fn check(file: &ast::File) -> (Program, Vec<Diagnostic>) {
    let mut checker = Checker {
        diagnostics: Vec::new(),
        globals: HashMap::new(),
        program: Program::default(),
        fn_signatures: Vec::new(),
        view_params: Vec::new(),
        fn_effects: FnEffects::default(),
        body: Body::new(Context::Ui),
        effects: Effects::default(),
    };
    checker.check_file(file);
    let mut diagnostics = checker.diagnostics;
    diagnostics.sort_by_key(|diagnostic| diagnostic.span.start);
    (checker.program, diagnostics)
}

impl Checker {
    fn check_file(&mut self, file: &ast::File) {
        self.declare_types(file);
        self.declare_signatures(file);
        let state_effects = self.check_states(file);
        self.check_functions(file);
        self.check_state_values(&state_effects);
        self.check_views(file);
        self.check_windows(file);
    }

    fn declare_types(&mut self, file: &ast::File) {
        let structs: Vec<&ast::StructDecl> = file
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Struct(decl) => Some(decl),
                _ => None,
            })
            .collect();
        for decl in &structs {
            let id = StructId(self.program.structs.len());
            self.declare_global(&decl.name, Global::Struct(id));
            self.program.structs.push(StructDef {
                name: decl.name.name.clone(),
                fields: Vec::new(),
            });
        }
        for (index, decl) in structs.iter().enumerate() {
            let mut fields: Vec<FieldDef> = Vec::new();
            for field in &decl.fields {
                if fields
                    .iter()
                    .any(|existing| existing.name == field.name.name)
                {
                    self.error(
                        format!("field `{}` is declared twice", field.name.name),
                        field.name.span,
                    );
                    continue;
                }
                let ty = self.resolve_type(&field.ty);
                fields.push(FieldDef {
                    name: field.name.name.clone(),
                    ty,
                });
            }
            self.program.structs[index].fields = fields;
        }
    }

    fn declare_signatures(&mut self, file: &ast::File) {
        for item in &file.items {
            match item {
                Item::Fn(decl) => {
                    let id = FnId(self.fn_signatures.len());
                    self.declare_global(&decl.name, Global::Fn(id));
                    let params = self.resolve_params(&decl.params);
                    let return_type = decl
                        .return_type
                        .as_ref()
                        .map_or(Type::Void, |ty| self.resolve_type(ty));
                    self.fn_signatures.push(Signature {
                        params,
                        return_type,
                    });
                }
                Item::View(decl) => {
                    let id = ViewId(self.view_params.len());
                    self.declare_global(&decl.name, Global::View(id));
                    let params = self.resolve_params(&decl.params);
                    self.view_params.push(params);
                }
                _ => {}
            }
        }
    }

    fn resolve_params(&mut self, params: &[ast::TypedName]) -> Vec<(String, Type)> {
        let mut resolved: Vec<(String, Type)> = Vec::new();
        for param in params {
            if resolved.iter().any(|(name, _)| *name == param.name.name) {
                self.error(
                    format!("parameter `{}` is declared twice", param.name.name),
                    param.name.span,
                );
            }
            let ty = self.resolve_type(&param.ty);
            resolved.push((param.name.name.clone(), ty));
        }
        resolved
    }

    /// Checks each state's starting value; returns their effects, which are validated once
    /// the functions they call have been checked.
    fn check_states(&mut self, file: &ast::File) -> Vec<(Span, Effects)> {
        let mut all_effects = Vec::new();
        for item in &file.items {
            let Item::State(decl) = item else { continue };
            let annotation = decl.ty.as_ref().map(|ty| self.resolve_type(ty));
            self.body = Body::new(Context::StateInit);
            let (init, effects) =
                self.measure(|checker| checker.expr(&decl.value, annotation.as_ref()));
            let ty = match annotation {
                Some(annotation) => {
                    self.require(&init.ty, &annotation, init.span);
                    annotation
                }
                None => init.ty.clone(),
            };
            if ty == Type::Void {
                self.error("this has no value to start with", init.span);
            }
            let id = StateId(self.program.states.len());
            self.declare_global(&decl.name, Global::State(id));
            self.program.states.push(StateDef {
                name: decl.name.name.clone(),
                ty,
                init,
            });
            all_effects.push((decl.value.span, effects));
        }
        all_effects
    }

    fn check_functions(&mut self, file: &ast::File) {
        let mut direct = Vec::new();
        for item in &file.items {
            let Item::Fn(decl) = item else { continue };
            let id = FnId(self.program.functions.len());
            let signature = &self.fn_signatures[id.0];
            let return_type = signature.return_type.clone();
            let params = signature.params.clone();
            self.body = Body::new(Context::Function {
                return_type: return_type.clone(),
            });
            for (name, ty) in &params {
                self.body.declare(
                    name,
                    LocalInfo {
                        ty: ty.clone(),
                        alias: false,
                    },
                );
            }
            let (body, effects) = self.measure(|checker| checker.block(&decl.body));
            if return_type != Type::Void && !stmt::always_returns(&body) {
                self.error(
                    format!(
                        "`{}` can reach its end without returning a `{}`",
                        decl.name.name,
                        self.type_name(&return_type)
                    ),
                    decl.name.span,
                );
            }
            self.program.functions.push(FnDef {
                name: decl.name.name.clone(),
                params: params.len(),
                return_type,
                body,
                locals: mem::take(&mut self.body.locals).len(),
            });
            direct.push(effects);
        }
        self.fn_effects = FnEffects::solve(&direct);
    }

    fn check_state_values(&mut self, state_effects: &[(Span, Effects)]) {
        for (span, effects) in state_effects {
            let (reads, writes) = self.fn_effects.total(effects);
            if !reads.is_empty() || writes {
                self.error("a state's starting value can't use other state", *span);
            }
        }
    }

    fn check_views(&mut self, file: &ast::File) {
        for item in &file.items {
            let Item::View(decl) = item else { continue };
            let params = self.view_params[self.program.views.len()].clone();
            self.body = Body::new(Context::Ui);
            for (name, ty) in &params {
                self.body.declare(
                    name,
                    LocalInfo {
                        ty: ty.clone(),
                        alias: false,
                    },
                );
            }
            let body = self.ui_block(&decl.body.stmts);
            self.program.views.push(ViewDef {
                name: decl.name.name.clone(),
                params: params.len(),
                body,
                locals: mem::take(&mut self.body.locals).len(),
            });
        }
    }

    fn check_windows(&mut self, file: &ast::File) {
        let mut top_level_span = None;
        for item in &file.items {
            let Item::Stmt(stmt) = item else { continue };
            top_level_span.get_or_insert(stmt.span);
            self.body = Body::new(Context::Ui);
            if let Some(window) = self.window(stmt) {
                self.program.windows.push(window);
            }
        }
        if self.program.windows.is_empty() {
            let span = top_level_span.unwrap_or_default();
            self.error(
                "a Nifty app needs a `window(\"Title\") { … }` to show",
                span,
            );
        }
    }

    fn declare_global(&mut self, name: &ast::Ident, global: Global) {
        let built_in = if elements::element(&name.name).is_some() {
            Some("element")
        } else if functions::function(&name.name).is_some() {
            Some("function")
        } else {
            None
        };
        if let Some(kind) = built_in {
            self.error(
                format!("`{}` is a built-in {kind}; choose another name", name.name),
                name.span,
            );
            return;
        }
        if self.globals.insert(name.name.clone(), global).is_some() {
            self.error(format!("`{}` is already declared", name.name), name.span);
        }
    }

    fn resolve_type(&mut self, ty: &TypeExpr) -> Type {
        match &ty.kind {
            TypeExprKind::List(element) => Type::List(Rc::new(self.resolve_type(element))),
            TypeExprKind::Named(name) => {
                if let Some(primitive) = Type::primitive(name) {
                    primitive
                } else if let Some(Global::Struct(id)) = self.globals.get(name) {
                    Type::Struct(*id)
                } else {
                    self.error(format!("unknown type `{name}`"), ty.span);
                    Type::Error
                }
            }
        }
    }

    /// Runs `check` with a fresh record of effects and returns what it read and wrote.
    fn measure<T>(&mut self, check: impl FnOnce(&mut Self) -> T) -> (T, Effects) {
        let outer = mem::take(&mut self.effects);
        let result = check(self);
        let inner = mem::replace(&mut self.effects, outer);
        self.effects.merge(&inner);
        (result, inner)
    }

    fn type_name(&self, ty: &Type) -> String {
        if let Some(name) = ty.primitive_name() {
            return name.to_owned();
        }
        match ty {
            Type::Int | Type::Float | Type::Bool | Type::String => {
                unreachable!("primitive types are named by `PRIMITIVE_TYPES`")
            }
            Type::List(element) => format!("[{}]", self.type_name(element)),
            Type::Struct(id) => self.program.structs[id.0].name.clone(),
            Type::Void => "nothing".to_owned(),
            Type::Error => "?".to_owned(),
        }
    }

    /// Reports a mismatch unless either side already has an error.
    fn require(&mut self, actual: &Type, expected: &Type, span: Span) {
        if actual != expected && !actual.is_error() && !expected.is_error() {
            self.error(
                format!(
                    "expected `{}`, found `{}`",
                    self.type_name(expected),
                    self.type_name(actual)
                ),
                span,
            );
        }
    }

    fn error(&mut self, message: impl Into<String>, span: Span) {
        self.diagnostics.push(Diagnostic::new(message, span));
    }
}
