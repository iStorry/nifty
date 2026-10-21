//! `download(…)` and `format_bytes(…)`, which hand their work to the runtime, and the
//! array of bindings `download` and `fetch` both pass.

use std::mem::{offset_of, size_of};

use cranelift_codegen::ir::types::{I8, I32};
use cranelift_codegen::ir::{InstBuilder, MemFlags, Value};
use cranelift_module::Module;
use nifty_check::hir::{Download, DownloadCommand, Expr, PlaceStep, Type};
use nifty_rt::{DownloadBinding, DownloadDesc};
use nifty_syntax::Span;

use super::{Body, Result, offset};

impl<M: Module> Body<'_, '_, '_, M> {
    /// Asks for a download once the event finishes. The places are worked out after the
    /// address, folder and settings, as in the interpreter, so each borrowed value is held
    /// across everything worked out after it.
    pub(super) fn download(&mut self, download: &Download, span: Span) -> Result<()> {
        let settings = [
            &download.folder,
            &download.name,
            &download.connections,
            &download.headers,
            &download.retries,
        ];
        let indexes = download
            .bindings
            .iter()
            .flat_map(|(_, place)| &place.path)
            .filter_map(|step| match step {
                PlaceStep::Index(index) => Some(index),
                PlaceStep::Field(_) => None,
            });
        let after = |setting: usize| settings[setting..].iter().copied().chain(indexes.clone());
        let url = self.expr(&download.url)?;
        let url = self.hold_across(url, &Type::String, after(0))?;
        let folder = self.expr(&download.folder)?;
        let folder = self.hold_across(folder, &Type::String, after(1))?;
        let name = self.expr(&download.name)?;
        let name = self.hold_across(name, &Type::String, after(2))?;
        let connections = self.int(&download.connections)?;
        let headers = self.expr(&download.headers)?;
        let headers = self.hold_across(headers, &download.headers.ty, after(4))?;
        let retries = self.int(&download.retries)?;
        let mut places = Vec::with_capacity(download.bindings.len());
        for (field, place) in &download.bindings {
            let path = self.path(place)?;
            let Some(desc) = self.place_desc(&path)? else {
                return self.fail_now(self.shared.runtime.fail_download_into_variable, span);
            };
            places.push((field.code(), desc));
        }
        let bindings = self.bindings_array(
            &places,
            size_of::<DownloadBinding>(),
            offset_of!(DownloadBinding, field),
            offset_of!(DownloadBinding, place),
        )?;
        let desc = self.stack(size_of::<DownloadDesc>())?;
        let len = self.usize(places.len())?;
        for (value, at) in [
            (url.value, offset_of!(DownloadDesc, url)),
            (folder.value, offset_of!(DownloadDesc, folder)),
            (name.value, offset_of!(DownloadDesc, name)),
            (connections, offset_of!(DownloadDesc, connections)),
            (headers.value, offset_of!(DownloadDesc, headers)),
            (retries, offset_of!(DownloadDesc, retries)),
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
        self.discard(folder, &Type::String)?;
        self.discard(name, &Type::String)?;
        self.discard(headers, &download.headers.ty)
    }

    /// Each field's code and place, as an array on the stack of bindings `size` bytes long
    /// with the field's code at `field_offset` and the place at `place_offset`, and its
    /// address: `nifty_rt::DownloadBinding`s or `nifty_rt::FetchBinding`s.
    pub(super) fn bindings_array(
        &mut self,
        places: &[(u8, Value)],
        size: usize,
        field_offset: usize,
        place_offset: usize,
    ) -> Result<Value> {
        let array = self.stack(places.len().max(1) * size)?;
        for (index, &(field, place)) in places.iter().enumerate() {
            let field = self.b.ins().iconst(I8, i64::from(field));
            let field_at = offset(index * size + field_offset)?;
            let place_at = offset(index * size + place_offset)?;
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
