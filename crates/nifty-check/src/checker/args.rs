//! Matching call arguments to parameters: positional ones by order, named ones by name.

use nifty_syntax::Span;
use nifty_syntax::ast::{Arg, Expr};

use super::Checker;
use crate::elements::Param;

pub(super) struct Args<'a> {
    callee: String,
    /// A built-in's parameters from its table, which every name the checker asks for must
    /// be in, so tools reading the table see the same arguments.
    declared: Option<&'static [Param]>,
    positional: Vec<&'a Expr>,
    positional_taken: usize,
    named: Vec<(&'a str, &'a Expr, Span, bool)>,
}

impl<'a> Args<'a> {
    /// `callee` names what is called in error messages, such as "`label`".
    pub(super) fn new(checker: &mut Checker, callee: String, args: &'a [Arg]) -> Self {
        let mut positional = Vec::new();
        let mut named: Vec<(&str, &Expr, Span, bool)> = Vec::new();
        for arg in args {
            match &arg.label {
                None => positional.push(&arg.value),
                Some(label) => {
                    if named.iter().any(|(name, ..)| *name == label.name) {
                        checker.error(format!("`{}` is given twice", label.name), label.span);
                    } else {
                        named.push((&label.name, &arg.value, label.span, false));
                    }
                }
            }
        }
        Self {
            callee,
            declared: None,
            positional,
            positional_taken: 0,
            named,
        }
    }

    /// Arguments to the built-in `name`, whose table entry declares `params`.
    pub(super) fn built_in(
        checker: &mut Checker,
        name: &str,
        params: &'static [Param],
        args: &'a [Arg],
    ) -> Self {
        Self {
            declared: Some(params),
            ..Self::new(checker, format!("`{name}`"), args)
        }
    }

    /// The next positional argument, if one was given.
    pub(super) fn positional(&mut self) -> Option<&'a Expr> {
        let arg = self.positional.get(self.positional_taken).copied();
        if arg.is_some() {
            self.positional_taken += 1;
        }
        arg
    }

    pub(super) fn named(&mut self, name: &str) -> Option<&'a Expr> {
        debug_assert!(
            self.declared
                .is_none_or(|params| params.iter().any(|param| param.name == name)),
            "{} reads `{name}`, which its table entry doesn't declare",
            self.callee
        );
        let entry = self.named.iter_mut().find(|(label, ..)| *label == name)?;
        entry.3 = true;
        Some(entry.1)
    }

    /// The parameter's argument, given by position or by name.
    pub(super) fn param(&mut self, name: &str) -> Option<&'a Expr> {
        self.positional().or_else(|| self.named(name))
    }

    /// Reports arguments nothing asked for.
    pub(super) fn finish(self, checker: &mut Checker) {
        for extra in &self.positional[self.positional_taken..] {
            checker.error(
                format!("{} doesn't take this many arguments", self.callee),
                extra.span,
            );
        }
        for (name, _, span, used) in &self.named {
            if !used {
                checker.error(
                    format!("{} has no argument named `{name}`", self.callee),
                    *span,
                );
            }
        }
    }
}
