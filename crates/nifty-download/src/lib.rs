//! `download(…)`, shared by the interpreter and compiled apps: each download runs on its own
//! thread and sends reports back over a channel, waking the UI thread, which writes them
//! wherever the app's call bound them.

mod fetch;
#[cfg(feature = "testing")]
pub mod testing;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use nifty_runtime::Waker;

use fetch::Abandoned;
pub use fetch::Report;

/// What `status` shows from the click until the download finishes.
pub const DOWNLOADING: &str = "Downloading…";
const DONE_PERCENT: i64 = 100;

/// A download an action asked for, started once the action has finished. `targets` says
/// where its reports go, in whatever terms the app uses.
#[derive(Debug)]
pub struct Request<T> {
    pub url: String,
    pub folder: String,
    pub targets: T,
}

impl Report {
    /// The percentage `progress` shows after this report, if it changes.
    #[must_use]
    pub fn progress(&self) -> Option<i64> {
        match self {
            Self::Progress(percent) => Some(*percent),
            Self::Saved(_) => Some(DONE_PERCENT),
            Self::Failed(_) => None,
        }
    }

    /// What `status` shows after this report, if it changes.
    #[must_use]
    pub fn status(&self) -> Option<String> {
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
pub struct Downloads<T> {
    waker: Arc<dyn Waker>,
    sender: Sender<(DownloadId, Report)>,
    receiver: Receiver<(DownloadId, Report)>,
    running: RefCell<HashMap<DownloadId, Running<T>>>,
    next_id: Cell<DownloadId>,
}

type Worker = JoinHandle<std::result::Result<(), Abandoned>>;

struct Running<T> {
    targets: T,
    /// Joined once its last report arrives; `None` when a test is already waiting on it.
    worker: Option<Worker>,
}

impl<T> Downloads<T> {
    /// `waker` asks the UI thread to call [`Downloads::receive`].
    pub fn new(waker: Arc<dyn Waker>) -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            waker,
            sender,
            receiver,
            running: RefCell::new(HashMap::new()),
            next_id: Cell::new(0),
        }
    }

    pub fn start(&self, request: Request<T>) {
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
    ///
    /// # Errors
    /// The first error `apply` returns.
    ///
    /// # Panics
    /// When a report arrives from a download this doesn't know, which can't happen: each
    /// is forgotten only after its last report.
    pub fn receive<E>(&self, mut apply: impl FnMut(&T, &Report) -> Result<(), E>) -> Result<(), E> {
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
    #[cfg(feature = "testing")]
    pub fn wait(&self) {
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

impl<T> Running<T> {
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

impl<T> std::fmt::Debug for Downloads<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Downloads")
            .field("running", &self.running.borrow().len())
            .finish_non_exhaustive()
    }
}
