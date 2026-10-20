//! `pick_file(…)` and the other file dialogs in compiled apps. The call names the state its
//! answer is bound to as a path from a state, which the answer is written through later.

use nifty_runtime::{DialogKind, Failure, FileDialog, Picked};

use crate::abi::borrow_context;
use crate::context::{AppError, Context};
use crate::place::{Location, PlaceDesc};
use crate::value::{self, LIST_OF_STRINGS, STRING, Slot};

/// Where a file dialog's answer goes, and the span of its call, which errors writing it
/// point at.
#[derive(Debug)]
pub(crate) struct Answer {
    place: Location,
    kind: DialogKind,
    span: u32,
}

impl Context {
    /// Writes the path or paths the user picked into the state the dialog's call bound;
    /// cancelling changes nothing.
    pub(crate) fn answer_dialog(&self, answer: &Answer, picked: Picked) -> Result<(), AppError> {
        let fail = |failure| self.error(&failure, answer.span);
        let paths = match picked {
            Picked::Chosen(paths) => paths,
            Picked::Cancelled => return Ok(()),
            Picked::Failed(reason) => return Err(fail(Failure::Dialog(reason))),
        };
        let written = if answer.kind.picks_many() {
            let items = paths.into_iter().map(value::string_slot).collect();
            self.write_location(
                &answer.place,
                value::list_slot(&LIST_OF_STRINGS, items),
                &LIST_OF_STRINGS,
            )
        } else {
            let path = paths
                .into_iter()
                .next()
                .expect("backends answer a one-path dialog with exactly one path");
            self.write_location(&answer.place, value::string_slot(path), &STRING)
        };
        written.map_err(fail)
    }
}

/// Asks for a file dialog of `kind`, shown once the current event finishes, whose answer
/// goes to the state at `answer`. `title`, `folder` and `name` are borrowed strings, and
/// `types` a borrowed list of strings; `""` and `[]` leave a setting to the platform.
///
/// # Safety
/// See the `abi` module's contract; the place's steps are readable.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn nf_pick(
    context: *const Context,
    kind: DialogKind,
    answer: &PlaceDesc,
    title: Slot,
    types: Slot,
    folder: Slot,
    name: Slot,
    span: u32,
) -> bool {
    // SAFETY: by the caller's contract.
    let context = unsafe { borrow_context(context) };
    // SAFETY: by the caller's contract.
    let place = unsafe { Location::from_place(answer) };
    let types: Vec<&str> = value::as_list(types)
        .items
        .iter()
        .map(|item| value::as_str(*item))
        .collect();
    let asked = place.and_then(|place| {
        let dialog = FileDialog::new(
            kind,
            value::as_str(title).to_owned(),
            &types,
            value::as_str(folder),
            value::as_str(name).to_owned(),
        )
        .map_err(|error| Failure::Dialog(error.to_string()))?;
        Ok((dialog, Answer { place, kind, span }))
    });
    match asked {
        Ok((dialog, answer)) => {
            context.ask_dialog(dialog, answer);
            true
        }
        Err(failure) => {
            context.fail(failure, span);
            false
        }
    }
}

/// Fails a file dialog whose answer is bound to a variable rather than state.
///
/// # Safety
/// See the `abi` module's contract.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn nf_fail_dialog_into_variable(context: *const Context, span: u32) {
    // SAFETY: by the caller's contract.
    unsafe { borrow_context(context) }.fail(Failure::DialogIntoVariable, span);
}
