//! Calls to built-in functions.

use std::rc::Rc;

use nifty_syntax::Span;
use nifty_syntax::ast::{self, Arg};

use super::args::Args;
use super::expr::reported;
use super::{Checker, Global};
use crate::elements::{BoundType, WINDOW};
use crate::functions::{DOWNLOAD, FROM, Function, FunctionKind, NAME, TITLE, TYPES};
use crate::hir::{Dialog, DialogKind, Download, Expr, ExprKind, Type, WindowAction, WindowId};

impl Checker {
    pub(super) fn built_in_call(
        &mut self,
        function: &'static Function,
        args: &[Arg],
        span: Span,
    ) -> Expr {
        let args = Args::built_in(self, function.name, function.params, args);
        match function.kind {
            FunctionKind::Download => self.download(args, span),
            FunctionKind::PickFile => self.dialog(function.name, DialogKind::File, args, span),
            FunctionKind::PickFiles => self.dialog(function.name, DialogKind::Files, args, span),
            FunctionKind::PickFolder => self.dialog(function.name, DialogKind::Folder, args, span),
            FunctionKind::PickSavePath => {
                self.dialog(function.name, DialogKind::SavePath, args, span)
            }
            FunctionKind::OpenWindow => {
                self.window_request(function.name, WindowAction::Open, args, span)
            }
            FunctionKind::CloseWindow => {
                self.window_request(function.name, WindowAction::Close, args, span)
            }
        }
    }

    /// `open_window(settings)` or `close_window(settings)`. Opening and closing windows
    /// changes the app, so the UI can't do it while it is shown.
    fn window_request(
        &mut self,
        name: &str,
        action: WindowAction,
        mut args: Args,
        span: Span,
    ) -> Expr {
        let window = if let Some(window) = args.positional() {
            self.window_name(window)
        } else {
            self.error(
                format!("`{name}` needs a window's name, such as `{name}(settings)`"),
                span,
            );
            None
        };
        args.finish(self);
        let Some(window) = window else {
            return reported(span);
        };
        self.effects.writes = true;
        Expr {
            kind: ExprKind::Window(action, window),
            ty: Type::Void,
            span,
        }
    }

    /// The window `expr` names.
    fn window_name(&mut self, expr: &ast::Expr) -> Option<WindowId> {
        let ast::ExprKind::Name(name) = &expr.kind else {
            self.error("write a window's name here, such as `settings`", expr.span);
            return None;
        };
        if let Some(Global::Window(window)) = self.global(name) {
            return Some(window);
        }
        self.error(
            format!(
                "`{name}` isn't the name of a window; name one with `{WINDOW} {name}(\"Title\") {{ … }}`"
            ),
            expr.span,
        );
        None
    }

    /// `download(url, to: folder, progress: $percent, status: $text)`. It changes the bound
    /// state later, from another thread, so it counts as writing both now.
    fn download(&mut self, mut args: Args, span: Span) -> Expr {
        let url = self.required_arg(args.positional(), &Type::String, span, || {
            format!(
                "`{DOWNLOAD}` needs the address to fetch, such as `{DOWNLOAD}(\"https://…\", …)`"
            )
        });
        let folder = self.required_arg(args.named("to"), &Type::String, span, || {
            format!("`{DOWNLOAD}` needs a folder to save into, such as `to: \"~/Downloads\"`")
        });
        let progress = self.bound_place(
            DOWNLOAD,
            Some("progress"),
            args.named("progress"),
            &Type::Int,
            span,
        );
        let status = self.bound_place(
            DOWNLOAD,
            Some("status"),
            args.named("status"),
            &Type::String,
            span,
        );
        args.finish(self);
        let (Some(progress), Some(status)) = (progress, status) else {
            return reported(span);
        };
        self.note_write(&progress);
        self.note_write(&status);
        Expr {
            kind: ExprKind::Download(Box::new(Download {
                url,
                folder,
                progress,
                status,
            })),
            ty: Type::Void,
            span,
        }
    }

    /// `pick_file($path, title: …, types: […], from: …)` and its siblings. The answer is
    /// written later, once the user closes the dialog, so it counts as writing it now.
    fn dialog(&mut self, name: &str, kind: DialogKind, mut args: Args, span: Span) -> Expr {
        let answer_type = if kind.picks_many() {
            BoundType::Strings
        } else {
            BoundType::String
        };
        let answer = self.bound_place(name, None, args.positional(), &answer_type.ty(), span);
        let title = self.optional_text(args.named(TITLE), span);
        let types = match kind {
            DialogKind::Folder => no_types(span),
            DialogKind::File | DialogKind::Files | DialogKind::SavePath => {
                let strings = BoundType::Strings.ty();
                args.named(TYPES)
                    .map_or_else(|| no_types(span), |types| self.expect(types, &strings))
            }
        };
        let file_name = match kind {
            DialogKind::SavePath => self.optional_text(args.named(NAME), span),
            DialogKind::File | DialogKind::Files | DialogKind::Folder => empty_text(span),
        };
        let folder = self.optional_text(args.named(FROM), span);
        args.finish(self);
        let Some(answer) = answer else {
            return reported(span);
        };
        self.note_write(&answer);
        Expr {
            kind: ExprKind::Dialog(Box::new(Dialog {
                kind,
                answer,
                title,
                types,
                folder,
                name: file_name,
            })),
            ty: Type::Void,
            span,
        }
    }

    fn optional_text(&mut self, arg: Option<&ast::Expr>, span: Span) -> Expr {
        arg.map_or_else(|| empty_text(span), |arg| self.expect(arg, &Type::String))
    }
}

fn empty_text(span: Span) -> Expr {
    Expr {
        kind: ExprKind::String(Rc::from("")),
        ty: Type::String,
        span,
    }
}

fn no_types(span: Span) -> Expr {
    Expr {
        kind: ExprKind::List(Vec::new()),
        ty: BoundType::Strings.ty(),
        span,
    }
}
