//! A download's own thread, which handles everything that happens to it, in order: its
//! connection's progress, and what the app asks of it.

use std::fmt;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use nifty_runtime::{DownloadCommand, NoHomeFolder, expand_home};

use crate::connection::{self, ConnectionError, Outlet};
use crate::http::{self, Answer};
use crate::name::{create_unique, file_name};
use crate::report::{Progress, Report};
use crate::speed::SpeedMeter;

/// How often progress is reported while data arrives, so a fast download doesn't flood
/// the UI thread with updates nobody can see.
const REPORT_INTERVAL: Duration = Duration::from_millis(100);

/// Something that happened to a download.
#[derive(Debug)]
pub(crate) enum Event {
    /// The app asked for this.
    Command(DownloadCommand),
    /// The app is gone, so nobody will hear from this download again.
    Abandoned,
    /// The server answered, and the body is on its way.
    Answered(Answer),
    /// This many more bytes are in the file.
    Wrote(u64),
    /// The connection reached the end of the body, or failed.
    Ended(Result<(), ConnectionError>),
}

/// What a download fetches, and where it saves it.
#[derive(Debug)]
pub(crate) struct Job {
    pub url: String,
    pub folder: String,
}

/// The UI thread is gone, so nobody is waiting for this download any more.
#[derive(Debug)]
pub(crate) struct Abandoned;

/// How a download ended, other than being saved.
#[derive(Debug)]
enum Stopped {
    Failed(DownloadError),
    Cancelled,
    Abandoned,
}

#[derive(Debug)]
enum DownloadError {
    Connection(ConnectionError),
    File(io::Error),
    NoHomeFolder(NoHomeFolder),
    /// The download stopped and its partly written file couldn't be removed.
    LeftBehind {
        reason: Box<Stopped>,
        path: PathBuf,
    },
}

impl fmt::Display for DownloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connection(error) => write!(f, "{error}"),
            Self::File(error) => write!(f, "{error}"),
            Self::NoHomeFolder(error) => write!(f, "{error}"),
            Self::LeftBehind { reason, path } => {
                let reason = match &**reason {
                    Stopped::Failed(error) => error.to_string(),
                    Stopped::Cancelled => "cancelled".to_owned(),
                    Stopped::Abandoned => "the app stopped waiting for it".to_owned(),
                };
                write!(
                    f,
                    "{reason}; the partial file {} is left behind",
                    path.display()
                )
            }
        }
    }
}

impl From<io::Error> for Stopped {
    fn from(error: io::Error) -> Self {
        Self::Failed(DownloadError::File(error))
    }
}

impl From<NoHomeFolder> for Stopped {
    fn from(error: NoHomeFolder) -> Self {
        Self::Failed(DownloadError::NoHomeFolder(error))
    }
}

impl From<Abandoned> for Stopped {
    fn from(Abandoned: Abandoned) -> Self {
        Self::Abandoned
    }
}

/// Runs `job`, handling `events` until it ends, and sending each step to `report`.
///
/// # Errors
/// [`Abandoned`] once the app is gone, which stops the download early.
pub(crate) fn run(
    job: &Job,
    events: &Receiver<Event>,
    connect: Sender<Event>,
    report: &impl Fn(Report) -> Result<(), Abandoned>,
) -> Result<(), Abandoned> {
    let outlet = Arc::new(Outlet::default());
    connection::spawn(http::agent(), job.url.clone(), Arc::clone(&outlet), connect);
    let mut download = Download {
        outlet,
        path: None,
        progress: Progress::default(),
        meter: SpeedMeter::default(),
        last_report: None,
    };
    let stopped = match download.follow(job, events, report) {
        Ok(saved) => return report(saved),
        Err(stopped) => download.clean_up(stopped),
    };
    match stopped {
        Stopped::Abandoned => Err(Abandoned),
        Stopped::Cancelled => report(Report::Cancelled),
        Stopped::Failed(error) => report(Report::Failed(error.to_string())),
    }
}

/// A download in progress, as its own thread sees it.
struct Download {
    outlet: Arc<Outlet>,
    /// The file being written, once the server has answered.
    path: Option<PathBuf>,
    progress: Progress,
    meter: SpeedMeter,
    last_report: Option<Instant>,
}

impl Download {
    /// Handles events until the file is saved, giving the report that says so, or the
    /// download stops.
    fn follow(
        &mut self,
        job: &Job,
        events: &Receiver<Event>,
        report: &impl Fn(Report) -> Result<(), Abandoned>,
    ) -> Result<Report, Stopped> {
        loop {
            // The app holds a sender until it lets go of the download.
            let Ok(event) = events.recv() else {
                return Err(Stopped::Abandoned);
            };
            match event {
                Event::Command(DownloadCommand::Cancel) => return Err(Stopped::Cancelled),
                Event::Abandoned => return Err(Stopped::Abandoned),
                Event::Answered(answer) => self.create(job, &answer)?,
                Event::Wrote(bytes) => self.wrote(bytes, report)?,
                Event::Ended(Ok(())) => return self.save(),
                Event::Ended(Err(error)) => {
                    return Err(Stopped::Failed(DownloadError::Connection(error)));
                }
            }
        }
    }

    /// Creates the file the server's answer names, and lets the connection write it.
    fn create(&mut self, job: &Job, answer: &Answer) -> Result<(), Stopped> {
        let folder = expand_home(&job.folder)?;
        fs::create_dir_all(&folder)?;
        let (path, file) = create_unique(&folder, &file_name(&answer.uri))?;
        self.path = Some(path);
        self.progress.total = answer.length;
        self.outlet.open(file);
        Ok(())
    }

    fn wrote(
        &mut self,
        bytes: u64,
        report: &impl Fn(Report) -> Result<(), Abandoned>,
    ) -> Result<(), Abandoned> {
        self.progress.received += bytes;
        let now = Instant::now();
        self.progress.speed = self.meter.record(now, self.progress.received);
        if self
            .last_report
            .is_none_or(|last| now.duration_since(last) >= REPORT_INTERVAL)
        {
            report(Report::Progress(self.progress))?;
            self.last_report = Some(now);
        }
        Ok(())
    }

    /// Makes sure what the connection wrote is on disk.
    fn save(&mut self) -> Result<Report, Stopped> {
        if let Some(file) = self.outlet.close() {
            file.sync_all()?;
        }
        let path = self
            .path
            .take()
            .expect("a body only ends after its answer created the file");
        Ok(Report::Saved {
            path,
            size: self.progress.received,
        })
    }

    /// Stops the connection and removes what it wrote, saying why the download stopped.
    fn clean_up(&mut self, stopped: Stopped) -> Stopped {
        self.outlet.close();
        let Some(path) = self.path.take() else {
            return stopped;
        };
        match fs::remove_file(&path) {
            Ok(()) => stopped,
            Err(_) => Stopped::Failed(DownloadError::LeftBehind {
                reason: Box::new(stopped),
                path,
            }),
        }
    }
}
