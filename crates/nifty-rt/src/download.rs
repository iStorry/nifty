//! `download(…)` in compiled apps. The call names the state its `progress:` and `status:`
//! are bound to as paths from a state, which the reports are written through later.

use nifty_download::{DOWNLOADING, Report};
use nifty_runtime::Failure;

use crate::abi::borrow_context;
use crate::context::{AppError, Context};
use crate::place::{Location, PlaceDesc};
use crate::value::{self, INT, STRING, Slot};

/// Where a download's reports go, and the span of its call, which errors writing them
/// point at.
#[derive(Debug)]
pub(crate) struct Targets {
    progress: Location,
    status: Location,
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
