//! `download(…)`, shared by the interpreter and compiled apps: each download runs on its own
//! thread and sends reports back over a channel, waking the UI thread, which writes them
//! wherever the app's call bound them.

mod connection;
mod follow;
mod http;
mod id;
mod name;
mod part;
mod report;
mod size;
mod speed;
#[cfg(feature = "testing")]
pub mod testing;
mod worker;

use std::cell::RefCell;
use std::collections::HashMap;
#[cfg(feature = "testing")]
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use nifty_runtime::{DownloadCommand, Waker};

pub use follow::{StatePlace, StateReader, follow_id};
pub use id::DownloadId;
pub use report::{Progress, Report, Written};
pub use size::format_bytes;
use worker::{Abandoned, Event, Job};

/// A download an action asked for, started once the action has finished. `targets` says
/// where its reports go, in whatever terms the app uses.
#[derive(Debug)]
pub struct Request<T> {
    pub id: DownloadId,
    pub url: String,
    pub folder: String,
    /// How many connections the call asked for; any number below one asks for one.
    pub connections: i64,
    pub targets: T,
}

/// The downloads an app has running, and the channel their reports arrive on.
pub struct Downloads<T> {
    waker: Arc<dyn Waker>,
    sender: Sender<(DownloadId, Report)>,
    receiver: Receiver<(DownloadId, Report)>,
    running: RefCell<HashMap<DownloadId, Running<T>>>,
    /// Reports a test waited for, handed out before any others.
    #[cfg(feature = "testing")]
    waited: RefCell<VecDeque<(DownloadId, Report)>>,
}

type Worker = JoinHandle<Result<(), Abandoned>>;

struct Running<T> {
    targets: T,
    /// Where the app's commands for it go.
    events: Sender<Event>,
    /// Whether the app still holds it. One whose item is gone is cancelled, and its last
    /// reports go nowhere.
    held: bool,
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
            #[cfg(feature = "testing")]
            waited: RefCell::new(VecDeque::new()),
        }
    }

    pub fn start(&self, request: Request<T>) {
        let Request {
            id,
            url,
            folder,
            connections,
            targets,
        } = request;
        let job = Job {
            url,
            folder,
            connections: part::connection_count(connections),
        };
        let (events, inbox) = mpsc::channel();
        let connect = events.clone();
        let sender = self.sender.clone();
        let waker = Arc::clone(&self.waker);
        let worker = thread::spawn(move || {
            worker::run(job, &inbox, connect, &|report| {
                sender.send((id, report)).map_err(|_| Abandoned)?;
                waker.wake();
                Ok(())
            })
        });
        self.running.borrow_mut().insert(
            id,
            Running {
                targets,
                events,
                held: true,
                worker: Some(worker),
            },
        );
    }

    /// Asks the download whose id `id` shows to do `command`. An id of no running download,
    /// such as one that has just finished, asks nothing.
    pub fn command(&self, id: &str, command: DownloadCommand) {
        let running = self.running.borrow();
        let download = DownloadId::parse(id).and_then(|id| running.get(&id));
        if let Some(download) = download.filter(|download| download.held) {
            download.send(Event::Command(command));
        }
    }

    /// Hands each report that has arrived to `apply` with where it goes, forgetting
    /// downloads that have finished. `follow` first moves a download's targets to where its
    /// item is now, or says the app no longer holds it, which cancels it. Stops at the
    /// first error, leaving later reports queued.
    ///
    /// # Errors
    /// The first error `apply` returns.
    ///
    /// # Panics
    /// When a report arrives from a download this doesn't know, which can't happen: each
    /// is forgotten only after its last report.
    pub fn receive<E>(
        &self,
        mut follow: impl FnMut(&mut T) -> bool,
        mut apply: impl FnMut(&T, &Report) -> Result<(), E>,
    ) -> Result<(), E> {
        while let Some((id, report)) = self.next_report() {
            let mut running = self.running.borrow_mut();
            let download = running
                .get_mut(&id)
                .expect("reports only come from downloads still running");
            download.keep_if_held(&mut follow);
            let applied = if download.held {
                apply(&download.targets, &report)
            } else {
                Ok(())
            };
            if report.is_final()
                && let Some(finished) = running.remove(&id)
            {
                finished.join();
            }
            applied?;
        }
        Ok(())
    }

    /// Cancels each download whose item `follow` no longer finds, after the app's code
    /// changed state, and keeps the rest's targets where their items are.
    pub fn cancel_unheld(&self, mut follow: impl FnMut(&mut T) -> bool) {
        for download in self.running.borrow_mut().values_mut() {
            download.keep_if_held(&mut follow);
        }
    }

    #[cfg(not(feature = "testing"))]
    fn next_report(&self) -> Option<(DownloadId, Report)> {
        self.receiver.try_recv().ok()
    }

    #[cfg(feature = "testing")]
    fn next_report(&self) -> Option<(DownloadId, Report)> {
        let waited = self.waited.borrow_mut().pop_front();
        waited.or_else(|| self.receiver.try_recv().ok())
    }

    /// Waits until a report arrives, keeps it for [`Downloads::receive`], and wakes the
    /// UI thread to receive it, in case the download hasn't yet.
    ///
    /// # Panics
    /// Never: this holds a sender, so receiving can't disconnect.
    #[cfg(feature = "testing")]
    pub fn wait_for_report(&self) {
        let report = self
            .receiver
            .recv()
            .expect("this holds a sender, so receiving never disconnects");
        self.waited.borrow_mut().push_back(report);
        self.waker.wake();
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
    /// Cancels this download if `follow` finds the app no longer holds it.
    fn keep_if_held(&mut self, follow: &mut impl FnMut(&mut T) -> bool) {
        if self.held && !follow(&mut self.targets) {
            self.held = false;
            self.send(Event::Command(DownloadCommand::Cancel));
        }
    }

    fn send(&self, event: Event) {
        // This only fails once the worker has sent its last report and stopped listening,
        // when there is nothing left to ask of it.
        let _ = self.events.send(event);
    }

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

/// The app is going, as when `nf dev` replaces its program: each download stops and
/// removes its partial file before this returns. A download's thread never waits on the
/// network, only on its events, so this is quick.
impl<T> Drop for Downloads<T> {
    fn drop(&mut self) {
        let running = self.running.get_mut();
        for download in running.values() {
            download.send(Event::Abandoned);
        }
        for worker in running
            .values_mut()
            .filter_map(|download| download.worker.take())
        {
            // Abandoned is how a worker ends once its app is gone, unless it had already
            // finished.
            let _: Result<(), Abandoned> = worker.join().expect("download workers don't panic");
        }
    }
}

impl<T> std::fmt::Debug for Downloads<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Downloads")
            .field("running", &self.running.borrow().len())
            .finish_non_exhaustive()
    }
}
