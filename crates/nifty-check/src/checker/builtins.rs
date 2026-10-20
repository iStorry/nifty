//! Calls to built-in functions.

use nifty_syntax::Span;
use nifty_syntax::ast::Arg;

use super::Checker;
use super::args::Args;
use super::expr::reported;
use crate::functions::{DOWNLOAD, Function, FunctionKind};
use crate::hir::{Download, Expr, ExprKind, Type};

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
        }
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
        let progress = self.bound_place(DOWNLOAD, args.named("progress"), &Type::Int, span);
        let status = self.bound_place(DOWNLOAD, args.named("status"), &Type::String, span);
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
}
