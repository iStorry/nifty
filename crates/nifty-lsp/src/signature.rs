//! The parameters of whatever a call names: a built-in element or function, or a function,
//! view or struct declared in any file of the project.

use nifty_check::elements::{self, ParamKind};
use nifty_check::functions;
use nifty_syntax::ast::{Item, TypedName};

use crate::document::to_u32;
use crate::project::Project;

#[derive(Debug)]
pub(crate) struct Signature {
    pub name: String,
    pub params: Vec<Param>,
    pub summary: Option<&'static str>,
}

#[derive(Debug)]
pub(crate) struct Param {
    pub name: String,
    /// How the expected value reads, such as `number` or `[Todo]`.
    pub detail: String,
    /// Whether it may be given without a label. Declared callables accept either form;
    /// built-ins take each parameter in exactly one.
    pub positional: bool,
    pub named: bool,
    /// The built-in parameter's kind; `None` for declared callables.
    pub kind: Option<ParamKind>,
}

impl Signature {
    /// `name(param: detail, …)`, with each parameter's place in it.
    pub(crate) fn label(&self) -> (String, Vec<[u32; 2]>) {
        let mut label = format!("{}(", self.name);
        let mut offsets = Vec::with_capacity(self.params.len());
        for (index, param) in self.params.iter().enumerate() {
            if index > 0 {
                label.push_str(", ");
            }
            let start = label.encode_utf16().count();
            label.push_str(&param.name);
            label.push_str(": ");
            label.push_str(&param.detail);
            offsets.push([to_u32(start), to_u32(label.encode_utf16().count())]);
        }
        label.push(')');
        (label, offsets)
    }

    /// The parameter an argument fills: the one its label names, or else the next one that
    /// may be given by position.
    pub(crate) fn active(&self, label: Option<&str>, positional: usize) -> Option<usize> {
        match label {
            Some(label) => self.params.iter().position(|param| param.name == label),
            None => self
                .params
                .iter()
                .enumerate()
                .filter(|(_, param)| param.positional)
                .nth(positional)
                .map(|(index, _)| index),
        }
    }
}

pub(crate) fn of(project: &Project, callee: &str) -> Option<Signature> {
    if let Some(element) = elements::element(callee) {
        return Some(built_in(element.name, element.params, element.summary));
    }
    if let Some(function) = functions::function(callee) {
        return Some(built_in(function.name, function.params, function.summary));
    }
    let params = project.items().find_map(|item| match item {
        Item::Fn(decl) if decl.name.name == callee => Some(&decl.params),
        Item::View(decl) if decl.name.name == callee => Some(&decl.params),
        Item::Struct(decl) if decl.name.name == callee => Some(&decl.fields),
        _ => None,
    })?;
    Some(Signature {
        name: callee.to_owned(),
        params: params
            .iter()
            .map(|param| declared(project, param))
            .collect(),
        summary: None,
    })
}

fn built_in(name: &str, params: &[elements::Param], summary: &'static str) -> Signature {
    Signature {
        name: name.to_owned(),
        params: params
            .iter()
            .map(|param| Param {
                name: param.name.to_owned(),
                detail: param.kind.describe().into_owned(),
                positional: param.positional,
                named: !param.positional,
                kind: Some(param.kind),
            })
            .collect(),
        summary: Some(summary),
    }
}

fn declared(project: &Project, param: &TypedName) -> Param {
    Param {
        name: param.name.name.clone(),
        detail: project.text(param.ty.span).to_owned(),
        positional: true,
        named: true,
        kind: None,
    }
}
