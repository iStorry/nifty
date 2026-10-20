//! Reading and changing places: state, locals, and the fields and list items inside them.

use std::rc::Rc;

use nifty_check::hir::{LocalId, Place, PlaceRoot, PlaceStep, StateId};
use nifty_download::{StatePlace, StateReader};
use nifty_runtime::Step;
use nifty_syntax::Span;

use super::{Frame, Machine, Result, Slot, item, out_of_range, position};
use crate::value::Value;

/// Where a value lives, with list indexes already evaluated.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Location {
    root: Root,
    path: Vec<Step>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Root {
    State(StateId),
    Local(LocalId),
}

impl Location {
    /// Whether this is part of the app's state, which outlives any one frame.
    pub(crate) fn is_in_state(&self) -> bool {
        matches!(self.root, Root::State(_))
    }

    /// The location of the list item at `index` inside this location.
    pub(crate) fn item(&self, index: usize) -> Self {
        let mut path = self.path.clone();
        path.push(Step::Index(index));
        Self {
            root: self.root,
            path,
        }
    }
}

impl StatePlace for Location {
    fn steps(&self) -> &[Step] {
        &self.path
    }

    fn with_steps(&self, steps: Vec<Step>) -> Self {
        Self {
            root: self.root,
            path: steps,
        }
    }

    fn same_root(&self, other: &Self) -> bool {
        self.root == other.root
    }
}

/// Reads state without a frame, for finding where a download's id went.
impl StateReader for Machine {
    type Place = Location;

    fn list_len(&self, place: &Location) -> Option<usize> {
        match self.read_in_state(place)? {
            Value::List(items) => Some(items.len()),
            _ => None,
        }
    }

    fn holds_text(&self, place: &Location, text: &str) -> bool {
        matches!(self.read_in_state(place), Some(Value::String(held)) if *held == *text)
    }
}

impl Machine {
    /// The value at `place` in state; `None` when it is a variable's, or no longer exists.
    fn read_in_state(&self, place: &Location) -> Option<Value> {
        let Root::State(state) = place.root else {
            return None;
        };
        walk(&self.states.borrow()[state.0], &place.path, Span::default()).ok()
    }

    /// Resolves `place` against `frame`, following `for` variables to the item they alias.
    pub(crate) fn locate(&self, place: &Place, frame: &mut Frame) -> Result<Location> {
        let mut location = match place.root {
            PlaceRoot::State(state) => Location {
                root: Root::State(state),
                path: Vec::new(),
            },
            PlaceRoot::Local(local) => match &frame[local.0] {
                Slot::Alias(location) => location.clone(),
                Slot::Value(_) => Location {
                    root: Root::Local(local),
                    path: Vec::new(),
                },
                Slot::Empty => panic!("the checker only allows using locals after they are set"),
            },
        };
        for step in &place.path {
            location.path.push(match step {
                PlaceStep::Field(index) => Step::Field(*index),
                PlaceStep::Index(index) => {
                    Step::Index(position(self.eval(index, frame)?.as_int(), place.span)?)
                }
            });
        }
        Ok(location)
    }

    pub(crate) fn read(&self, location: &Location, frame: &Frame, span: Span) -> Result<Value> {
        match location.root {
            Root::State(state) => walk(&self.states.borrow()[state.0], &location.path, span),
            Root::Local(local) => match &frame[local.0] {
                Slot::Value(value) => walk(value, &location.path, span),
                _ => panic!("a local location always roots at a stored value"),
            },
        }
    }

    pub(crate) fn write(
        &self,
        location: &Location,
        frame: &mut Frame,
        value: Value,
        span: Span,
    ) -> Result<()> {
        self.modify(location, frame, span, |target| {
            *target = value;
            Ok(())
        })
    }

    /// Writes to a location in state, which needs no frame.
    ///
    /// # Panics
    /// When the location is a variable's; check [`Location::is_in_state`] first.
    pub(crate) fn write_state(&self, location: &Location, value: Value, span: Span) -> Result<()> {
        let Root::State(state) = location.root else {
            panic!("only a location in state can be written without its frame");
        };
        self.modify_state(state, &location.path, span, |target| {
            *target = value;
            Ok(())
        })
    }

    /// Changes the value at `location` in place, marking its state changed.
    pub(crate) fn modify<R>(
        &self,
        location: &Location,
        frame: &mut Frame,
        span: Span,
        change: impl FnOnce(&mut Value) -> Result<R>,
    ) -> Result<R> {
        match location.root {
            Root::State(state) => self.modify_state(state, &location.path, span, change),
            Root::Local(local) => match &mut frame[local.0] {
                Slot::Value(value) => change(walk_mut(value, &location.path, span)?),
                _ => panic!("a local location always roots at a stored value"),
            },
        }
    }

    fn modify_state<R>(
        &self,
        state: StateId,
        path: &[Step],
        span: Span,
        change: impl FnOnce(&mut Value) -> Result<R>,
    ) -> Result<R> {
        let result = {
            let mut states = self.states.borrow_mut();
            change(walk_mut(&mut states[state.0], path, span)?)?
        };
        self.changed.borrow_mut().insert(state);
        Ok(result)
    }
}

fn walk(value: &Value, path: &[Step], span: Span) -> Result<Value> {
    let Some((step, rest)) = path.split_first() else {
        return Ok(value.clone());
    };
    match (step, value) {
        (Step::Field(index), Value::Struct(fields)) => walk(&fields[*index], rest, span),
        (Step::Index(position), list) => walk(item(list, *position, span)?, rest, span),
        (Step::Field(_), other) => panic!("checked code read a field of {other:?}"),
    }
}

fn walk_mut<'v>(value: &'v mut Value, path: &[Step], span: Span) -> Result<&'v mut Value> {
    let Some((step, rest)) = path.split_first() else {
        return Ok(value);
    };
    match (step, value) {
        (Step::Field(index), Value::Struct(fields)) => {
            walk_mut(&mut Rc::make_mut(fields)[*index], rest, span)
        }
        (Step::Index(position), Value::List(items)) => {
            let len = items.len();
            let item = Rc::make_mut(items)
                .get_mut(*position)
                .ok_or_else(|| out_of_range(*position, len, span))?;
            walk_mut(item, rest, span)
        }
        (_, other) => panic!("checked code changed part of {other:?}"),
    }
}
