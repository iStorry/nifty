//! Which values survive a reload: a state keeps its value when the new program still declares
//! it with the same name and a type whose values are laid out the same way.

use nifty_check::hir::{Program, StateDef, StructId, Type};

use super::Machine;
use crate::value::Value;

/// The value `state`, declared by the program `next`, keeps from `previous`, if any.
pub(super) fn kept_value(previous: &Machine, state: &StateDef, next: &Program) -> Option<Value> {
    let program = &previous.program;
    let index = program
        .states
        .iter()
        .position(|old| old.name == state.name)?;
    let mut types = SameTypes {
        old: program,
        new: next,
        assumed: Vec::new(),
    };
    types
        .same(&program.states[index].ty, &state.ty)
        .then(|| previous.states.borrow()[index].clone())
}

/// Compares types across two programs, where struct ids differ but names and fields decide.
struct SameTypes<'a> {
    old: &'a Program,
    new: &'a Program,
    /// Struct pairs being compared further up, taken as equal so recursive structs end.
    assumed: Vec<(StructId, StructId)>,
}

impl SameTypes<'_> {
    fn same(&mut self, old: &Type, new: &Type) -> bool {
        match (old, new) {
            (Type::Int, Type::Int)
            | (Type::Float, Type::Float)
            | (Type::Bool, Type::Bool)
            | (Type::String, Type::String) => true,
            (Type::List(old), Type::List(new)) => self.same(old, new),
            (Type::Struct(old), Type::Struct(new)) => self.same_struct(*old, *new),
            _ => false,
        }
    }

    fn same_struct(&mut self, old_id: StructId, new_id: StructId) -> bool {
        if self.assumed.contains(&(old_id, new_id)) {
            return true;
        }
        let (old, new) = (self.old, self.new);
        let old = &old.structs[old_id.0];
        let new = &new.structs[new_id.0];
        if old.name != new.name || old.fields.len() != new.fields.len() {
            return false;
        }
        self.assumed.push((old_id, new_id));
        let same = old
            .fields
            .iter()
            .zip(&new.fields)
            .all(|(old, new)| old.name == new.name && self.same(&old.ty, &new.ty));
        self.assumed.pop();
        same
    }
}
