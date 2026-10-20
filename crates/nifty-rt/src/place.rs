//! Places in state that compiled code names for writes that happen later, such as where a
//! download reports or a file dialog answers: a state, then a path of fields and items.

use std::slice;

use nifty_runtime::Failure;

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
enum Step {
    Field(usize),
    Index(usize),
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

impl Context {
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
