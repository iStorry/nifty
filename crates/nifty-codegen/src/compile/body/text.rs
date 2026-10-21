//! The methods only strings have, and `json_text`, which hand their work to the runtime.

use cranelift_codegen::ir::types::I64;
use cranelift_codegen::ir::{InstBuilder, Value};
use cranelift_module::{FuncId, Module};
use nifty_check::hir::{Expr, TextMethod};
use nifty_syntax::Span;

use super::{Body, Result};

impl<M: Module> Body<'_, '_, '_, M> {
    /// A method only strings have: an owned value, or a whole number or `bool` in a slot.
    pub(super) fn text_method(
        &mut self,
        method: TextMethod,
        receiver: &Expr,
        args: &[Expr],
        span: Span,
    ) -> Result<Value> {
        let exprs: Vec<&Expr> = std::iter::once(receiver).chain(args).collect();
        let operands = self.held_all(&exprs)?;
        let values: Vec<Value> = operands.iter().map(|operand| operand.value).collect();
        let runtime = &self.shared.runtime;
        let result = match method {
            TextMethod::Contains => self.flag(runtime.text_contains, &values),
            TextMethod::StartsWith => self.flag(runtime.text_starts_with, &values),
            TextMethod::EndsWith => self.flag(runtime.text_ends_with, &values),
            TextMethod::Find => self.checked_result(runtime.text_find, &values, span)?,
            TextMethod::Slice => self.checked_result(runtime.text_slice, &values, span)?,
            TextMethod::Split => self.call1(runtime.text_split, &values),
            TextMethod::Replace => self.call1(runtime.text_replace, &values),
            TextMethod::Trim => self.call1(runtime.text_trim, &values),
        };
        for (operand, expr) in operands.into_iter().zip(exprs) {
            self.discard(operand, &expr.ty)?;
        }
        Ok(result)
    }

    /// `json_text(json, path)`: an owned string.
    pub(super) fn json_text(&mut self, json: &Expr, path: &Expr) -> Result<Value> {
        let operands = self.held_all(&[json, path])?;
        let json_text = self.shared.runtime.json_text;
        let text = self.call1(json_text, &[operands[0].value, operands[1].value]);
        self.discard(operands[0], &json.ty)?;
        self.discard(operands[1], &path.ty)?;
        Ok(text)
    }

    /// A runtime call that answers `bool`, as a slot.
    fn flag(&mut self, function: FuncId, args: &[Value]) -> Value {
        let answer = self.call1(function, args);
        self.b.ins().uextend(I64, answer)
    }

    /// A runtime call that takes the context first, then `args` and the span, writes its
    /// result to the pointer after them, and can fail.
    fn checked_result(&mut self, function: FuncId, args: &[Value], span: Span) -> Result<Value> {
        let out = self.scratch();
        let span = self.span(span)?;
        let mut values = vec![self.context];
        values.extend_from_slice(args);
        values.extend([span, out]);
        self.call_checked(function, &values);
        Ok(self.load(out))
    }
}
