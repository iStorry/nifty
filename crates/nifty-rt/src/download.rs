//! `download(…)` in compiled apps. The call names the state its `progress:` and `status:`
//! are bound to as paths from a state, which the reports are written through later.

use std::slice;

use nifty_download::{DOWNLOADING, Report};
use nifty_runtime::Failure;

use crate::abi::borrow_context;
use crate::context::{AppError, Context};
use crate::value::{self, INT, STRING, Slot, TypeDesc};

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

/// Where a download's reports go, and the span of its call, which errors writing them
/// point at.
#[derive(Debug)]
pub(crate) struct Targets {
    progress: Location,
    status: Location,
    span: u32,
}

pub(crate) type Request = nifty_download::Request<Targets>;

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
    unsafe fn from_place(place: &PlaceDesc) -> Result<Self, Failure> {
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
    fn write_location(
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

    /// Writes a download's report into the state its call bound.
    pub(crate) fn report_download(
        &self,
        targets: &Targets,
        report: &Report,
    ) -> Result<(), AppError> {
        let fail = |failure| self.error(&failure, targets.span);
        if let Some(percent) = report.progress() {
            self.write_location(&targets.progress, value::int_slot(percent), &INT)
                .map_err(fail)?;
        }
        if let Some(status) = report.status() {
            self.write_location(&targets.status, value::string_slot(status), &STRING)
                .map_err(fail)?;
        }
        Ok(())
    }
}

/// Asks for `url`, borrowed, to be downloaded into `folder`, borrowed, once the current
/// event finishes, and shows it has begun: the state at `progress` becomes 0 and the state
/// at `status` says it is downloading. Its reports later go to the same places.
///
/// # Safety
/// See the `abi` module's contract; each place's steps are readable.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn nf_download(
    context: *const Context,
    url: Slot,
    folder: Slot,
    progress: &PlaceDesc,
    status: &PlaceDesc,
    span: u32,
) -> bool {
    // SAFETY: by the caller's contract.
    let context = unsafe { borrow_context(context) };
    // SAFETY: by the caller's contract.
    let places = unsafe { (Location::from_place(progress), Location::from_place(status)) };
    let started = places.0.and_then(|progress| {
        let status = places.1?;
        context.write_location(&progress, value::int_slot(0), &INT)?;
        context.write_location(&status, value::string_slot(DOWNLOADING), &STRING)?;
        Ok(Targets {
            progress,
            status,
            span,
        })
    });
    match started {
        Ok(targets) => {
            context.request_download(Request {
                url: value::as_str(url).to_owned(),
                folder: value::as_str(folder).to_owned(),
                targets,
            });
            true
        }
        Err(failure) => {
            context.fail(failure, span);
            false
        }
    }
}

/// Fails a `download` whose `progress:` or `status:` is a variable rather than state.
///
/// # Safety
/// See the `abi` module's contract.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn nf_fail_download_into_variable(context: *const Context, span: u32) {
    // SAFETY: by the caller's contract.
    unsafe { borrow_context(context) }.fail(Failure::DownloadIntoVariable, span);
}
