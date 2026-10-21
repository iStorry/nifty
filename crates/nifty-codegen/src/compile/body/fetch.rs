//! `fetch(…) { … }`, which hands the request to the runtime with its block compiled as an
//! action over a copy of the caller's locals.

use std::mem::{offset_of, size_of};
use std::rc::Rc;

use cranelift_codegen::ir::types::{I8, I64};
use cranelift_codegen::ir::{AbiParam, InstBuilder, MemFlags, Value};
use cranelift_module::Module;
use nifty_check::hir::{Block as Stmts, Fetch, LocalId, Type};
use nifty_rt::{FetchBinding, FetchDesc};
use nifty_syntax::Span;

use super::{Body, Result, offset};
use crate::CodegenError;
use crate::compile::DeferredAction;

impl<M: Module> Body<'_, '_, '_, M> {
    /// Asks for a fetch once the event finishes. Its bindings are state without list
    /// positions, so nothing worked out after the address can change it.
    pub(super) fn fetch(&mut self, fetch: &Fetch, span: Span) -> Result<()> {
        let url = self.expr(&fetch.url)?;
        let url = self.hold(url, &Type::String, &fetch.headers)?;
        let headers = self.expr(&fetch.headers)?;
        let mut places = Vec::with_capacity(fetch.bindings.len());
        for (field, place) in &fetch.bindings {
            let path = self.path(place)?;
            let desc = self.place_desc(&path)?.ok_or_else(|| {
                CodegenError::new("`fetch` binds only state, which the checker ensures")
            })?;
            places.push((field.code(), desc));
        }
        let bindings = self.bindings_array(
            &places,
            size_of::<FetchBinding>(),
            offset_of!(FetchBinding, field),
            offset_of!(FetchBinding, place),
        )?;
        let (action, frame, slots) = if let Some(block) = &fetch.action {
            self.deferred_action(block, span)?
        } else {
            let none = self.b.ins().iconst(I64, 0);
            (none, none, none)
        };
        let desc = self.stack(size_of::<FetchDesc>())?;
        let len = self.usize(places.len())?;
        for (value, at) in [
            (url.value, offset_of!(FetchDesc, url)),
            (headers.value, offset_of!(FetchDesc, headers)),
            (bindings, offset_of!(FetchDesc, bindings)),
            (len, offset_of!(FetchDesc, len)),
            (action, offset_of!(FetchDesc, action)),
            (frame, offset_of!(FetchDesc, frame)),
            (slots, offset_of!(FetchDesc, slots)),
        ] {
            let at = offset(at)?;
            self.b.ins().store(MemFlags::trusted(), value, desc, at);
        }
        let span = self.span(span)?;
        let start = self.shared.runtime.fetch;
        self.call_checked(start, &[self.context, desc, span]);
        self.discard(url, &Type::String)?;
        self.discard(headers, &fetch.headers.ty)
    }

    /// The block's action function and its frame's description, compiled once this
    /// function is, and the caller's locals to copy into that frame.
    fn deferred_action(&mut self, block: &Stmts, span: Span) -> Result<(Value, Value, Value)> {
        let mut signature = self.module.make_signature();
        signature
            .params
            .extend([AbiParam::new(I64), AbiParam::new(I64)]);
        signature.returns.push(AbiParam::new(I8));
        let function = self.module.declare_anonymous_function(&signature)?;
        let frame = self.module.declare_anonymous_data(false, false)?;
        self.shared.deferred.push(DeferredAction {
            function,
            frame,
            block: Rc::clone(block),
            locals: self.locals.to_vec(),
        });
        let callee = self.module.declare_func_in_func(function, self.b.func);
        let action = self.b.ins().func_addr(I64, callee);
        let frame = self.address(frame);
        let slots = self.captured_slots(span)?;
        Ok((action, frame, slots))
    }

    /// Each local's slot, borrowed, with a `for` variable over a place holding its item as
    /// it is now, since the item may move before the block runs.
    fn captured_slots(&mut self, span: Span) -> Result<Value> {
        let mut values = Vec::with_capacity(self.locals.len());
        for local in 0..self.locals.len() {
            let value = if let Some(item) = self.aliases.get(&LocalId(local)).cloned() {
                self.read(&item, span)?
            } else {
                let slot = self.slot(local)?;
                self.load(slot)
            };
            values.push(value);
        }
        self.array(&values)
    }
}
