//! `download(…)`: each download runs on its own thread and sends reports back over a channel,
//! waking the UI thread, which writes them into the state the call bound.

mod fetch;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use nifty_runtime::Waker;
use nifty_syntax::Span;

use crate::machine::{Location, Result};
use fetch::Abandoned;
pub(crate) use fetch::Report;

/// What `status` shows from the click until the download finishes.
pub(crate) const DOWNLOADING: &str = "Downloading…";
const DONE_PERCENT: i64 = 100;

/// Where a download's reports go: the state its `progress:` and `status:` bound.
#[derive(Debug)]
pub(crate) struct Targets {
    pub progress: Location,
    pub status: Location,
    /// The `download(…)` call, which errors writing the reports point at.
    pub span: Span,
}

/// A download an action asked for, started once the action has finished.
#[derive(Debug)]
pub(crate) struct Request {
    pub url: String,
    pub folder: String,
    pub targets: Targets,
}

impl Report {
    /// The percentage `progress` shows after this report, if it changes.
    pub(crate) fn progress(&self) -> Option<i64> {
        match self {
            Self::Progress(percent) => Some(*percent),
            Self::Saved(_) => Some(DONE_PERCENT),
            Self::Failed(_) => None,
        }
    }

    /// What `status` shows after this report, if it changes.
    pub(crate) fn status(&self) -> Option<String> {
        match self {
            Self::Progress(_) => None,
            Self::Saved(path) => Some(format!("Saved to {}", path.display())),
            Self::Failed(reason) => Some(format!("Failed: {reason}")),
        }
    }

    fn is_final(&self) -> bool {
        !matches!(self, Self::Progress(_))
    }
}

type DownloadId = usize;

/// The downloads an app has running, and the channel their reports arrive on.
pub(crate) struct Downloads {
    waker: Arc<dyn Waker>,
    sender: Sender<(DownloadId, Report)>,
    receiver: Receiver<(DownloadId, Report)>,
    running: RefCell<HashMap<DownloadId, Running>>,
    next_id: Cell<DownloadId>,
}

type Worker = JoinHandle<std::result::Result<(), Abandoned>>;

struct Running {
    targets: Targets,
    /// Joined once its last report arrives; `None` when a test is already waiting on it.
    worker: Option<Worker>,
}

impl Downloads {
    /// `waker` asks the UI thread to call [`Downloads::receive`].
    pub(crate) fn new(waker: Arc<dyn Waker>) -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            waker,
            sender,
            receiver,
            running: RefCell::new(HashMap::new()),
            next_id: Cell::new(0),
        }
    }

    pub(crate) fn start(&self, request: Request) {
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        let Request {
            url,
            folder,
            targets,
        } = request;
        let sender = self.sender.clone();
        let waker = Arc::clone(&self.waker);
        let worker = thread::spawn(move || {
            fetch::run(&url, &folder, &|report| {
                sender.send((id, report)).map_err(|_| Abandoned)?;
                waker.wake();
                Ok(())
            })
        });
        self.running.borrow_mut().insert(
            id,
            Running {
                targets,
                worker: Some(worker),
            },
        );
    }

    /// Hands each report that has arrived to `apply` with where it goes, forgetting
    /// downloads that have finished. Stops at the first error, leaving later reports queued.
    pub(crate) fn receive(
        &self,
        mut apply: impl FnMut(&Targets, &Report) -> Result<()>,
    ) -> Result<()> {
        for (id, report) in self.receiver.try_iter() {
            let mut running = self.running.borrow_mut();
            let download = running
                .get(&id)
                .expect("reports only come from downloads still running");
            let applied = apply(&download.targets, &report);
            if report.is_final()
                && let Some(finished) = running.remove(&id)
            {
                finished.join();
            }
            applied?;
        }
        Ok(())
    }

    /// Waits until every running download's worker has sent its last report.
    #[cfg(test)]
    pub(crate) fn wait(&self) {
        let workers: Vec<_> = self
            .running
            .borrow_mut()
            .values_mut()
            .filter_map(|download| download.worker.take())
            .collect();
        for worker in workers {
            join(worker);
        }
    }
}

impl Running {
    /// Waits for the worker, which exits right after sending its last report.
    fn join(self) {
        if let Some(worker) = self.worker {
            join(worker);
        }
    }
}

fn join(worker: Worker) {
    let finished = worker.join().expect("download workers don't panic");
    assert!(
        finished.is_ok(),
        "a worker is only abandoned once its app is gone"
    );
}

impl std::fmt::Debug for Downloads {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Downloads")
            .field("running", &self.running.borrow().len())
            .finish_non_exhaustive()
    }
}
