//! `download(…)` and `format_bytes(…)` in compiled apps. A download call names the state
//! each of its bindings is bound to as paths from a state, which the reports are written
//! through later.

use std::slice;

use nifty_download::{Report, Written};
use nifty_runtime::{DownloadField, Failure};

use crate::abi::borrow_context;
use crate::context::{AppError, Context};
use crate::place::{Location, PlaceDesc};
use crate::value::{self, INT, STRING, Slot};

/// One binding of a `download(…)` call: the report field, and the state it goes to.
#[derive(Debug)]
#[repr(C)]
pub struct DownloadBinding {
    pub field: DownloadField,
    pub place: *const PlaceDesc,
}

/// A `download(…)` call as compiled code builds it: the address and folder, borrowed, and
/// `len` bindings at `bindings`.
#[derive(Debug)]
#[repr(C)]
pub struct DownloadDesc {
    pub url: Slot,
    pub folder: Slot,
    pub bindings: *const DownloadBinding,
    pub len: usize,
}

/// Where a download's reports go, and the span of its call, which errors writing them
/// point at.
#[derive(Debug)]
pub(crate) struct Targets {
    bindings: Vec<(DownloadField, Location)>,
    span: u32,
}

pub(crate) type Request = nifty_download::Request<Targets>;

impl Context {
    /// Writes a download's report into the state its call bound.
    pub(crate) fn report_download(
        &self,
        targets: &Targets,
        report: &Report,
    ) -> Result<(), AppError> {
        self.write_report(targets, report)
            .map_err(|failure| self.error(&failure, targets.span))
    }

    fn write_report(&self, targets: &Targets, report: &Report) -> Result<(), Failure> {
        for (field, written) in report.writes() {
            let bound = targets.bindings.iter().find(|(bound, _)| *bound == field);
            if let Some((_, location)) = bound {
                match written {
                    Written::Int(number) => {
                        self.write_location(location, value::int_slot(number), &INT)?;
                    }
                    Written::Text(text) => {
                        self.write_location(location, value::string_slot(text), &STRING)?;
                    }
                }
            }
        }
        Ok(())
    }
}

/// Asks for the download `desc` describes once the current event finishes, and shows it
/// has begun in the state it binds. Its reports later go to the same places.
///
/// # Safety
/// See the `abi` module's contract; `desc.bindings` points to `desc.len` bindings, and
/// each binding's place and its steps are readable.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn nf_download(
    context: *const Context,
    desc: &DownloadDesc,
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
            let targets = Targets { bindings, span };
            context.write_report(&targets, &Report::Started)?;
            Ok(targets)
        });
    match started {
        Ok(targets) => {
            context.request_download(Request {
                url: value::as_str(desc.url).to_owned(),
                folder: value::as_str(desc.folder).to_owned(),
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

/// Fails a `download` with a binding to a variable rather than state.
///
/// # Safety
/// See the `abi` module's contract.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn nf_fail_download_into_variable(context: *const Context, span: u32) {
    // SAFETY: by the caller's contract.
    unsafe { borrow_context(context) }.fail(Failure::DownloadIntoVariable, span);
}

/// `format_bytes(bytes)`: the size as text, owned by the caller.
#[unsafe(no_mangle)]
pub(crate) extern "C" fn nf_format_bytes(bytes: i64) -> Slot {
    value::string_slot(nifty_download::format_bytes(bytes))
}
