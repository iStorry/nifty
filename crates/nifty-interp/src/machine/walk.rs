//! Stepping through what a `for` loops over, shared by loop statements and `for` regions.

use std::rc::Rc;

use nifty_check::hir::{Iterable, Loop};
use nifty_syntax::Span;

use super::{Frame, Location, Machine, Result, Slot};
use crate::value::Value;

/// A loop's iterable, worked out once before the first time round.
pub(crate) enum Walk {
    /// The length is read each time round, so a loop body may change the list.
    Place(Location, Span),
    Items(Rc<Vec<Value>>),
    Range {
        start: i64,
        end: i64,
    },
}

impl Machine {
    pub(crate) fn walk(&self, iterable: &Iterable, frame: &mut Frame) -> Result<Walk> {
        Ok(match iterable {
            Iterable::Place(place) => Walk::Place(self.locate(place, frame)?, place.span),
            Iterable::Value(list) => match self.eval(list, frame)? {
                Value::List(items) => Walk::Items(items),
                other => panic!("the checker only allows looping over lists, not {other:?}"),
            },
            Iterable::Range { start, end } => Walk::Range {
                start: self.eval(start, frame)?.as_int(),
                end: self.eval(end, frame)?.as_int(),
            },
        })
    }

    /// Puts the loop's variables for the round at `position` in `frame`, or returns
    /// `false` once the walk is over.
    pub(crate) fn enter_round(
        &self,
        head: &Loop,
        walk: &Walk,
        (position, index): (usize, i64),
        frame: &mut Frame,
    ) -> Result<bool> {
        let item = match walk {
            Walk::Place(location, span) => {
                if position >= list_len(&self.read(location, frame, *span)?) {
                    return Ok(false);
                }
                Slot::Alias(location.item(position))
            }
            Walk::Items(items) => match items.get(position) {
                Some(item) => Slot::Value(item.clone()),
                None => return Ok(false),
            },
            Walk::Range { start, end } => match start.checked_add(index) {
                Some(count) if count < *end => Slot::Value(Value::Int(count)),
                _ => return Ok(false),
            },
        };
        frame[head.item.0] = item;
        if let Some(local) = head.index {
            frame[local.0] = Slot::Value(Value::Int(index));
        }
        Ok(true)
    }
}

/// Each round's list position, and the same position as an `int` for the loop's index.
pub(crate) fn rounds() -> impl Iterator<Item = (usize, i64)> {
    (0..).zip(0..)
}

fn list_len(list: &Value) -> usize {
    match list {
        Value::List(items) => items.len(),
        other => panic!("the checker only allows looping over lists, not {other:?}"),
    }
}
