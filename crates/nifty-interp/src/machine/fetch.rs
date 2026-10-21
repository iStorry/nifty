//! `fetch(…) { … }`: asking for a page, and writing the response into state before running
//! the call's block with the locals it saw.

use nifty_check::hir::{Block, Fetch, FetchField};
use nifty_download::Written;
use nifty_download::fetch::{FetchRequest, Fetched};
use nifty_syntax::Span;

use super::{Frame, Location, Machine, Result, Slot, written_value};

/// Where a fetch's response goes, and what runs once it is written.
#[derive(Debug)]
pub(crate) struct FetchTargets {
    bindings: Vec<(FetchField, Location)>,
    /// The call's block, and the frame it runs in: the caller's locals as they were at the
    /// call.
    action: Option<(Block, Frame)>,
    /// The `fetch(…)` call, which errors writing the response point at.
    span: Span,
}

impl Machine {
    /// Queues a fetch for the app to start once the current event finishes, and shows that
    /// it has begun.
    pub(super) fn request_fetch(&self, fetch: &Fetch, frame: &mut Frame, span: Span) -> Result<()> {
        let url = self.eval(&fetch.url, frame)?.as_str().to_owned();
        let headers = self.eval(&fetch.headers, frame)?;
        let headers = headers.as_strs().into_iter().map(str::to_owned).collect();
        let bindings = fetch
            .bindings
            .iter()
            .map(|(field, place)| Ok((*field, self.locate(place, frame)?)))
            .collect::<Result<Vec<_>>>()?;
        let action = match &fetch.action {
            Some(block) => Some((block.clone(), self.capture(frame, span)?)),
            None => None,
        };
        let targets = FetchTargets {
            bindings,
            action,
            span,
        };
        self.write_fetched(&targets, Fetched::started())?;
        self.fetches.borrow_mut().push(FetchRequest {
            url,
            headers,
            targets,
        });
        Ok(())
    }

    /// Writes what a fetch got back into the state its call bound, then runs its block.
    pub(crate) fn answer_fetch(&self, targets: FetchTargets, fetched: &Fetched) -> Result<()> {
        self.write_fetched(&targets, fetched.writes())?;
        if let Some((action, mut frame)) = targets.action {
            self.exec_block(&action, &mut frame)?;
        }
        Ok(())
    }

    fn write_fetched(
        &self,
        targets: &FetchTargets,
        writes: Vec<(FetchField, Written)>,
    ) -> Result<()> {
        for (field, written) in writes {
            let bound = targets.bindings.iter().find(|(bound, _)| *bound == field);
            if let Some((_, location)) = bound {
                self.write_state(location, written_value(written), targets.span)?;
            }
        }
        Ok(())
    }

    /// A copy of `frame` that keeps each local's value as it is now: a `for` variable over
    /// a place holds its item's value rather than where the item lives, which may change
    /// before the copy is used.
    fn capture(&self, frame: &Frame, span: Span) -> Result<Frame> {
        frame
            .iter()
            .map(|slot| match slot {
                Slot::Alias(location) => Ok(Slot::Value(self.read(location, frame, span)?)),
                Slot::Empty | Slot::Value(_) => Ok(slot.clone()),
            })
            .collect()
    }
}
