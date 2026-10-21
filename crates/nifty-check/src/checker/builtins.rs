//! Calls to built-in functions.

use std::rc::Rc;

use nifty_syntax::Span;
use nifty_syntax::ast::{self, Arg};

use super::args::Args;
use super::expr::reported;
use super::{Checker, Global};
use crate::elements::{BoundType, TrailingBlock, WINDOW};
use crate::functions::{
    CONNECTIONS, DEFAULT_CONNECTIONS, DEFAULT_RETRIES, DOWNLOAD, FETCH, FORMAT_BYTES, FROM,
    Function, FunctionKind, HEADERS, JSON_TEXT, NAME, RETRIES, TITLE, TYPES, download_bound_type,
    fetch_bound_type,
};
use crate::hir::{
    Dialog, DialogKind, Download, DownloadCommand, DownloadField, Expr, ExprKind, Fetch,
    FetchField, Place, PlaceRoot, PlaceStep, Type, WindowAction, WindowId,
};

impl Checker {
    pub(super) fn built_in_call(
        &mut self,
        function: &'static Function,
        args: &[Arg],
        block: Option<&ast::Block>,
        span: Span,
    ) -> Expr {
        let args = Args::built_in(self, function.name, function.params, args);
        if function.block == TrailingBlock::None {
            self.no_block(function.name, block);
        }
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
            FunctionKind::FormatBytes => self.format_bytes(function, args, span),
            FunctionKind::Fetch => self.fetch(args, block, span),
            FunctionKind::JsonText => self.json_text(function, args, span),
            FunctionKind::DownloadCommand(command) => self.download_command(command, args, span),
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

    /// `download(url, to: folder, progress: $percent, status: $text, …)`, with settings it
    /// leaves out at their defaults. It changes the
    /// bound state later, from another thread, so it counts as writing all of it now.
    fn download(&mut self, mut args: Args, span: Span) -> Expr {
        let url = self.required_arg(args.positional(), &Type::String, span, || {
            format!(
                "`{DOWNLOAD}` needs the address to fetch, such as `{DOWNLOAD}(\"https://…\", …)`"
            )
        });
        let folder = self.required_arg(args.named("to"), &Type::String, span, || {
            format!("`{DOWNLOAD}` needs a folder to save into, such as `to: \"~/Downloads\"`")
        });
        let name = self.optional_text(args.named(NAME), span);
        let connections = self.optional_int(args.named(CONNECTIONS), DEFAULT_CONNECTIONS, span);
        let headers = self.optional_texts(args.named(HEADERS), span);
        let retries = self.optional_int(args.named(RETRIES), DEFAULT_RETRIES, span);
        let mut bindings = Vec::new();
        let mut all_bound = true;
        for &field in DownloadField::ALL {
            let arg = args.named(field.name());
            if arg.is_none() && !field.is_required() {
                continue;
            }
            let ty = download_bound_type(field).ty();
            match self.bound_place(DOWNLOAD, Some(field.name()), arg, &ty, span) {
                Some(place) => bindings.push((field, place)),
                None => all_bound = false,
            }
        }
        args.finish(self);
        if !all_bound {
            return reported(span);
        }
        let has_id = bindings
            .iter()
            .any(|(field, _)| *field == DownloadField::Id);
        if !has_id
            && bindings
                .iter()
                .any(|(_, place)| Self::is_in_list_item(place))
        {
            self.error(
                format!(
                    "`{DOWNLOAD}` reports into a list item here, so it needs `{id}: $…` in the same item, such as `{id}: $items[last].{id}`; its reports then stay with that item when the list changes",
                    id = DownloadField::Id.name()
                ),
                span,
            );
            return reported(span);
        }
        for (_, place) in &bindings {
            self.note_write(place);
        }
        Expr {
            kind: ExprKind::Download(Box::new(Download {
                url,
                folder,
                name,
                connections,
                headers,
                retries,
                bindings,
            })),
            ty: Type::Void,
            span,
        }
    }

    /// `cancel_download(id)` and the like. Asking changes what the download writes to
    /// state, so it counts as writing.
    fn download_command(&mut self, command: DownloadCommand, mut args: Args, span: Span) -> Expr {
        let name = command.function();
        let id = self.required_arg(args.positional(), &Type::String, span, || {
            format!("`{name}` needs a download's id, such as `{name}(item.id)`")
        });
        args.finish(self);
        self.effects.writes = true;
        Expr {
            kind: ExprKind::DownloadCommand(command, Box::new(id)),
            ty: Type::Void,
            span,
        }
    }

    /// `format_bytes(n)`, which only reads, so the UI may show it.
    fn format_bytes(&mut self, function: &Function, mut args: Args, span: Span) -> Expr {
        let bytes = self.required_arg(args.positional(), &Type::Int, span, || {
            format!("`{FORMAT_BYTES}` needs a number of bytes, such as `{FORMAT_BYTES}(1500)`")
        });
        args.finish(self);
        Expr {
            kind: ExprKind::FormatBytes(Box::new(bytes)),
            ty: function.value.map_or(Type::Void, BoundType::ty),
            span,
        }
    }

    /// `fetch(url, status: $s, …) { … }`. It writes the bound state later, from another
    /// thread, so it counts as writing all of it now; its block's effects are the caller's,
    /// checked where it is written.
    fn fetch(&mut self, mut args: Args, block: Option<&ast::Block>, span: Span) -> Expr {
        let url = self.required_arg(args.positional(), &Type::String, span, || {
            format!("`{FETCH}` needs the address to read, such as `{FETCH}(\"https://…\", …)`")
        });
        let headers = self.optional_texts(args.named(HEADERS), span);
        let mut bindings = Vec::new();
        let mut all_bound = true;
        for &field in FetchField::ALL {
            let arg = args.named(field.name());
            if arg.is_none() && !field.is_required() {
                continue;
            }
            let ty = fetch_bound_type(field).ty();
            match self.bound_place(FETCH, Some(field.name()), arg, &ty, span) {
                Some(place) if Self::is_in_list_item(&place) => {
                    self.error(
                        format!(
                            "`{FETCH}` writes its response into state just before its block runs, so `{}` can't bind a list item or a `for` variable; bind state such as `$page`, and have the block copy what an item needs",
                            field.name()
                        ),
                        place.span,
                    );
                    all_bound = false;
                }
                Some(place) => bindings.push((field, place)),
                None => all_bound = false,
            }
        }
        args.finish(self);
        let action = block.map(|block| self.action(block));
        if !all_bound {
            return reported(span);
        }
        for (_, place) in &bindings {
            self.note_write(place);
        }
        Expr {
            kind: ExprKind::Fetch(Box::new(Fetch {
                url,
                headers,
                bindings,
                action,
            })),
            ty: Type::Void,
            span,
        }
    }

    /// `json_text(json, path)`, which only reads, so the UI may show it.
    fn json_text(&mut self, function: &Function, mut args: Args, span: Span) -> Expr {
        let json = self.required_arg(args.positional(), &Type::String, span, || {
            format!("`{JSON_TEXT}` needs the JSON to read, such as `{JSON_TEXT}(page, [\"id\"])`")
        });
        let strings = BoundType::Strings.ty();
        let path = self.required_arg(args.positional(), &strings, span, || {
            format!(
                "`{JSON_TEXT}` needs the keys and positions to follow, such as `{JSON_TEXT}(json, [\"items\", \"0\", \"id\"])`"
            )
        });
        args.finish(self);
        Expr {
            kind: ExprKind::JsonText(Box::new(json), Box::new(path)),
            ty: function.value.map_or(Type::Void, BoundType::ty),
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
            DialogKind::Folder => no_texts(span),
            DialogKind::File | DialogKind::Files | DialogKind::SavePath => {
                self.optional_texts(args.named(TYPES), span)
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

    /// Whether the bound `place` is inside a list's item, whose position can change. A
    /// bound place rooted in a local is a `for` variable over a list's items.
    fn is_in_list_item(place: &Place) -> bool {
        matches!(place.root, PlaceRoot::Local(_))
            || place
                .path
                .iter()
                .any(|step| matches!(step, PlaceStep::Index(_)))
    }

    fn optional_text(&mut self, arg: Option<&ast::Expr>, span: Span) -> Expr {
        arg.map_or_else(|| empty_text(span), |arg| self.expect(arg, &Type::String))
    }

    /// A `[string]` setting, empty when left out.
    fn optional_texts(&mut self, arg: Option<&ast::Expr>, span: Span) -> Expr {
        let strings = BoundType::Strings.ty();
        arg.map_or_else(|| no_texts(span), |arg| self.expect(arg, &strings))
    }

    fn optional_int(&mut self, arg: Option<&ast::Expr>, default: i64, span: Span) -> Expr {
        arg.map_or_else(
            || Expr {
                kind: ExprKind::Int(default),
                ty: Type::Int,
                span,
            },
            |arg| self.expect(arg, &Type::Int),
        )
    }
}

fn empty_text(span: Span) -> Expr {
    Expr {
        kind: ExprKind::String(Rc::from("")),
        ty: Type::String,
        span,
    }
}

fn no_texts(span: Span) -> Expr {
    Expr {
        kind: ExprKind::List(Vec::new()),
        ty: BoundType::Strings.ty(),
        span,
    }
}
