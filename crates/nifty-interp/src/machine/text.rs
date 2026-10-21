//! The methods only strings have, and `json_text`.

use std::rc::Rc;

use nifty_check::hir::{Expr, TextMethod};
use nifty_syntax::Span;

use super::{Frame, Machine, Result, RuntimeError};
use crate::value::Value;

impl Machine {
    pub(super) fn text_method(
        &self,
        method: TextMethod,
        receiver: &Expr,
        args: &[Expr],
        frame: &mut Frame,
        span: Span,
    ) -> Result<Value> {
        let text = self.eval(receiver, frame)?;
        let text = text.as_str();
        let args = self.eval_all(args, frame)?;
        let string = |position: usize| args[position].as_str();
        let failed = |failure| RuntimeError::new(&failure, span);
        Ok(match method {
            TextMethod::Contains => Value::Bool(text.contains(string(0))),
            TextMethod::StartsWith => Value::Bool(text.starts_with(string(0))),
            TextMethod::EndsWith => Value::Bool(text.ends_with(string(0))),
            TextMethod::Find => Value::Int(nifty_runtime::find(text, string(0)).map_err(failed)?),
            TextMethod::Slice => Value::String(Rc::from(
                nifty_runtime::slice(text, args[0].as_int(), args[1].as_int()).map_err(failed)?,
            )),
            TextMethod::Split => Value::strings(nifty_runtime::split(text, string(0))),
            TextMethod::Replace => {
                Value::String(Rc::from(nifty_runtime::replace(text, string(0), string(1))))
            }
            TextMethod::Trim => Value::String(Rc::from(text.trim())),
        })
    }

    pub(super) fn json_text(&self, json: &Expr, path: &Expr, frame: &mut Frame) -> Result<Value> {
        let json = self.eval(json, frame)?;
        let path = self.eval(path, frame)?;
        Ok(Value::String(Rc::from(nifty_runtime::json_text(
            json.as_str(),
            &path.as_strs(),
        ))))
    }
}
