//! Places in state that compiled code names for writes that happen later, such as where a
//! download reports or a file dialog answers: a state, then a path of fields and items.

use std::slice;

use nifty_download::{StatePlace, StateReader};
use nifty_runtime::{Failure, Step};

use crate::context::Context;
use crate::value::{self, Slot, TypeDesc};

/// Where a value lives inside a state, with list positions already checked not to be
/// negative.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Location {
    state: usize,
    path: Vec<Step>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum StepKind {
    Field,
    Index,
}

/// One step of a path: a struct's field, or a list's item, by position.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct PathStep {
    pub kind: StepKind,
    pub index: i64,
}

/// A place in state, as compiled code builds one for a call: state `state`, then `len`
/// steps at `steps`.
#[derive(Debug)]
#[repr(C)]
pub struct PlaceDesc {
    pub state: u32,
    pub steps: *const PathStep,
    pub len: usize,
}

impl Location {
    /// # Safety
    /// `place.steps` points to `place.len` readable steps.
    pub(crate) unsafe fn from_place(place: &PlaceDesc) -> Result<Self, Failure> {
        let steps = if place.len == 0 {
            &[]
        } else {
            // SAFETY: by the caller's contract.
            unsafe { slice::from_raw_parts(place.steps, place.len) }
        };
        let path = steps
            .iter()
            .map(|step| {
                let index = usize::try_from(step.index)
                    .map_err(|_| Failure::NegativePosition(step.index))?;
                Ok(match step.kind {
                    StepKind::Field => Step::Field(index),
                    StepKind::Index => Step::Index(index),
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            state: place.state as usize,
            path,
        })
    }
}

impl StatePlace for Location {
    fn steps(&self) -> &[Step] {
        &self.path
    }

    fn with_steps(&self, steps: Vec<Step>) -> Self {
        Self {
            state: self.state,
            path: steps,
        }
    }

    fn same_root(&self, other: &Self) -> bool {
        self.state == other.state
    }
}

/// A context's state, read for finding where a download's id went. Download bindings are
/// typed by the checker, so the place above a list position holds a list, and an id's
/// place a string.
pub(crate) struct StateOf<'a>(pub &'a Context);

impl StateReader for StateOf<'_> {
    type Place = Location;

    fn list_len(&self, place: &Location) -> Option<usize> {
        Some(value::as_list(self.0.read_location(place)?).items.len())
    }

    fn holds_text(&self, place: &Location, text: &str) -> bool {
        self.0
            .read_location(place)
            .is_some_and(|slot| value::as_str(slot) == text)
    }
}

impl Context {
    /// The value at `location`, borrowed; `None` once a list position on the way is gone.
    fn read_location(&self, location: &Location) -> Option<Slot> {
        let mut slot = self.state(location.state);
        for step in &location.path {
            slot = match *step {
                Step::Field(index) => value::as_struct(slot).fields[index],
                Step::Index(position) => *value::as_list(slot).items.get(position)?,
            };
        }
        Some(slot)
    }

    /// Replaces the value at `location` with `value`, of type `ty`, which it takes
    /// ownership of, and marks its state changed.
    pub(crate) fn write_location(
        &self,
        location: &Location,
        value: Slot,
        ty: &TypeDesc,
    ) -> Result<(), Failure> {
        // SAFETY: state slots stay valid while the context lives, and nothing else holds
        // one while the UI thread runs this.
        let mut slot = unsafe { &mut *self.state_mut(location.state) };
        for step in &location.path {
            slot = match *step {
                Step::Field(index) => &mut value::struct_mut(slot).fields[index],
                Step::Index(position) => {
                    let items = &mut value::list_mut(slot).items;
                    let len = items.len();
                    let Some(item) = items.get_mut(position) else {
                        value::release(value, ty);
                        return Err(Failure::NoItem { position, len });
                    };
                    item
                }
            };
        }
        value::release(std::mem::replace(slot, value), ty);
        Ok(())
    }
}
