//! `download(…)` and `format_bytes(…)`, which hand their work to the runtime.

use std::mem::{offset_of, size_of};

use cranelift_codegen::ir::types::{I8, I32};
use cranelift_codegen::ir::{InstBuilder, MemFlags, Value};
use cranelift_module::Module;
use nifty_check::hir::{Download, DownloadCommand, Expr, Type};
use nifty_rt::{DownloadBinding, DownloadDesc};
use nifty_syntax::Span;

use super::{Body, Result, offset};

impl<M: Module> Body<'_, '_, '_, M> {
    /// Asks for a download once the event finishes. The places are worked out after the
    /// address, folder and settings, as in the interpreter.
    pub(super) fn download(&mut self, download: &Download, span: Span) -> Result<()> {
        let url = self.expr(&download.url)?;
        let url = self.hold(url, &Type::String, &download.folder)?;
        let folder = self.expr(&download.folder)?;
        let folder = self.hold(folder, &Type::String, &download.connections)?;
        let connections = self.int(&download.connections)?;
        let mut places = Vec::with_capacity(download.bindings.len());
        for (field, place) in &download.bindings {
            let path = self.path(place)?;
            let Some(desc) = self.place_desc(&path)? else {
                return self.fail_now(self.shared.runtime.fail_download_into_variable, span);
            };
            places.push((field.code(), desc));
        }
        let bindings = self.download_bindings(&places)?;
        let desc = self.stack(size_of::<DownloadDesc>())?;
        let len = self.usize(places.len())?;
        for (value, at) in [
            (url.value, offset_of!(DownloadDesc, url)),
            (folder.value, offset_of!(DownloadDesc, folder)),
            (connections, offset_of!(DownloadDesc, connections)),
            (bindings, offset_of!(DownloadDesc, bindings)),
            (len, offset_of!(DownloadDesc, len)),
        ] {
            let at = offset(at)?;
            self.b.ins().store(MemFlags::trusted(), value, desc, at);
        }
        let span = self.span(span)?;
        let start = self.shared.runtime.download;
        self.call_checked(start, &[self.context, desc, span]);
        self.discard(url, &Type::String)?;
        self.discard(folder, &Type::String)
    }

    /// Each field's code and place, as an array of `nifty_rt::DownloadBinding`s on the
    /// stack, and its address.
    fn download_bindings(&mut self, places: &[(u8, Value)]) -> Result<Value> {
        let size = size_of::<DownloadBinding>();
        let array = self.stack(places.len().max(1) * size)?;
        for (index, &(field, place)) in places.iter().enumerate() {
            let field = self.b.ins().iconst(I8, i64::from(field));
            let field_at = offset(index * size + offset_of!(DownloadBinding, field))?;
            let place_at = offset(index * size + offset_of!(DownloadBinding, place))?;
            self.b
                .ins()
                .store(MemFlags::trusted(), field, array, field_at);
            self.b
                .ins()
                .store(MemFlags::trusted(), place, array, place_at);
        }
        Ok(array)
    }

    /// `cancel_download(id)` and the like: asks for it, done once the event finishes.
    pub(super) fn download_command(&mut self, command: DownloadCommand, id: &Expr) -> Result<()> {
        let id_operand = self.expr(id)?;
        let command = self.b.ins().iconst(I32, i64::from(command as u32));
        let ask = self.shared.runtime.download_command;
        self.call(ask, &[self.context, command, id_operand.value]);
        self.discard(id_operand, &Type::String)
    }

    /// `format_bytes(n)`: an owned string.
    pub(super) fn format_bytes(&mut self, bytes: &Expr) -> Result<Value> {
        let bytes = self.int(bytes)?;
        let format = self.shared.runtime.format_bytes;
        Ok(self.call1(format, &[bytes]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bindings_field_is_stored_as_one_byte() {
        assert_eq!(size_of::<nifty_check::hir::DownloadField>(), 1);
    }
}
