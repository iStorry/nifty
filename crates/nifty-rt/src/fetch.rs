//! `fetch(…) { … }` in compiled apps. A call names the state each binding writes as a path
//! from a state, and hands over its block as a compiled action with a copy of the caller's
//! locals, which runs once the response is written.

use std::slice;

use nifty_download::fetch::Fetched;
use nifty_runtime::{Failure, FetchField};

use crate::abi::borrow_context;
use crate::context::{AppError, Context};
use crate::logic::Frame;
use crate::place::{Location, PlaceDesc};
use crate::program::{ActionFn, FrameDesc};
use crate::value::{self, Slot};

/// One binding of a `fetch(…)` call: the response's field, and the state it goes to.
#[derive(Debug)]
#[repr(C)]
pub struct FetchBinding {
    pub field: FetchField,
    pub place: *const PlaceDesc,
}

/// A `fetch(…)` call as compiled code builds it: the address and list of headers,
/// borrowed, `len` bindings at `bindings`, and its block, if it has one, with the
/// caller's locals, borrowed, at `slots`, as `frame` describes them.
#[derive(Debug)]
#[repr(C)]
pub struct FetchDesc {
    pub url: Slot,
    pub headers: Slot,
    pub bindings: *const FetchBinding,
    pub len: usize,
    pub action: Option<ActionFn>,
    pub frame: *const FrameDesc,
    pub slots: *const Slot,
}

/// Where a fetch's response goes, what runs then, and the span of its call, which errors
/// writing the response point at.
#[derive(Debug)]
pub(crate) struct FetchTargets {
    bindings: Vec<(FetchField, Location)>,
    action: Option<(ActionFn, Frame)>,
    span: u32,
}

pub(crate) type FetchRequest = nifty_download::fetch::FetchRequest<FetchTargets>;

impl Context {
    /// Writes what a fetch got back into the state its call bound; then the caller runs
    /// the action this gives back, if the call has a block.
    pub(crate) fn answer_fetch(
        &self,
        targets: FetchTargets,
        fetched: &Fetched,
    ) -> Result<Option<(ActionFn, Frame)>, AppError> {
        self.write_fetched(&targets.bindings, fetched.writes())
            .map_err(|failure| self.error(&failure, targets.span))?;
        Ok(targets.action)
    }

    fn write_fetched(
        &self,
        bindings: &[(FetchField, Location)],
        writes: Vec<(FetchField, nifty_download::Written)>,
    ) -> Result<(), Failure> {
        for (field, written) in writes {
            if let Some((_, location)) = bindings.iter().find(|(bound, _)| *bound == field) {
                self.write_written(location, written)?;
            }
        }
        Ok(())
    }
}

/// Asks for the fetch `desc` describes once the current event finishes, and shows it has
/// begun in the state it binds.
///
/// # Safety
/// See the `abi` module's contract; `desc.bindings` points to `desc.len` bindings, each
/// binding's place and its steps are readable, and when `desc.action` is set, `desc.frame`
/// describes the slots at `desc.slots`.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn nf_fetch(
    context: *const Context,
    desc: &FetchDesc,
    span: u32,
) -> bool {
    // SAFETY: by the caller's contract.
    let context = unsafe { borrow_context(context) };
    let bindings = if desc.len == 0 {
        &[]
    } else {
        // SAFETY: by the caller's contract.
        unsafe { slice::from_raw_parts(desc.bindings, desc.len) }
    };
    let started = bindings
        .iter()
        .map(|binding| {
            // SAFETY: by the caller's contract.
            let place = unsafe { Location::from_place(&*binding.place) }?;
            Ok((binding.field, place))
        })
        .collect::<Result<Vec<_>, Failure>>()
        .and_then(|bindings| {
            context.write_fetched(&bindings, Fetched::started())?;
            Ok(bindings)
        });
    let bindings = match started {
        Ok(bindings) => bindings,
        Err(failure) => {
            context.fail(failure, span);
            return false;
        }
    };
    let action = desc.action.map(|action| {
        // SAFETY: by the caller's contract.
        let frame = unsafe { Frame::copied(&*desc.frame, desc.slots) };
        (action, frame)
    });
    context.request_fetch(FetchRequest {
        url: value::as_str(desc.url).to_owned(),
        headers: value::as_list(desc.headers)
            .items
            .iter()
            .map(|header| value::as_str(*header).to_owned())
            .collect(),
        targets: FetchTargets {
            bindings,
            action,
            span,
        },
    });
    true
}
