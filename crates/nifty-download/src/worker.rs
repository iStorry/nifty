//! A download's own thread, which handles everything that happens to it, in order: its
//! connection's progress, and what the app asks of it.
//!
//! Pausing closes the connection's outlet and keeps the partial file; resuming opens a new
//! connection that asks for the rest with an HTTP range request. A server that ignores the
//! range sends the whole file again, so the download starts over from its first byte.

use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use nifty_runtime::{DownloadCommand, NoHomeFolder, expand_home};

use crate::connection::{self, ConnectionError, ConnectionId, Dial, Outlet};
use crate::http::{self, Answer};
use crate::name::{Partial, file_name};
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
    /// The server answered a connection, and the body is on its way.
    Answered(ConnectionId, Answer),
    /// A connection put this many more bytes in the file.
    Wrote(ConnectionId, u64),
    /// A connection reached the end of the body, or failed.
    Ended(ConnectionId, Result<(), ConnectionError>),
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
    /// The server sent a part of the file after the part already saved.
    PartTooFar {
        starts_at: u64,
        saved: u64,
    },
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
            Self::PartTooFar { starts_at, saved } => write!(
                f,
                "the server sent the file from byte {starts_at}, but only {saved} bytes are saved"
            ),
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
/// `connect` is where its connections send their events.
///
/// # Errors
/// [`Abandoned`] once the app is gone, which stops the download early.
pub(crate) fn run(
    job: Job,
    events: &Receiver<Event>,
    connect: Sender<Event>,
    report: &impl Fn(Report) -> Result<(), Abandoned>,
) -> Result<(), Abandoned> {
    let mut download = Download {
        job,
        connect,
        connection: 0,
        outlet: Arc::new(Outlet::default()),
        offset: 0,
        partial: None,
        progress: Progress::default(),
        paused: false,
        meter: SpeedMeter::default(),
        last_report: None,
    };
    download.dial();
    let stopped = match download.follow(events, report) {
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
    job: Job,
    connect: Sender<Event>,
    /// The connection running now, or that ran last while paused.
    connection: ConnectionId,
    outlet: Arc<Outlet>,
    /// Where in the file the connection running now writes its first byte.
    offset: u64,
    /// The file being written, once a server has answered.
    partial: Option<Partial>,
    progress: Progress,
    paused: bool,
    meter: SpeedMeter,
    last_report: Option<Instant>,
}

impl Download {
    /// Handles events until the file is saved, giving the report that says so, or the
    /// download stops.
    fn follow(
        &mut self,
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
                Event::Command(DownloadCommand::Pause) => self.pause(report)?,
                Event::Command(DownloadCommand::Resume) => {
                    if let Some(saved) = self.resume(report)? {
                        return Ok(saved);
                    }
                }
                Event::Abandoned => return Err(Stopped::Abandoned),
                // A connection stopped by a pause may still be heard from.
                Event::Answered(id, _) | Event::Wrote(id, _) | Event::Ended(id, _)
                    if id != self.connection || self.paused => {}
                Event::Answered(_, answer) => self.answered(&answer)?,
                Event::Wrote(_, bytes) => self.wrote(bytes, report)?,
                Event::Ended(_, Ok(())) => return self.save(),
                Event::Ended(_, Err(error)) => {
                    return Err(Stopped::Failed(DownloadError::Connection(error)));
                }
            }
        }
    }

    /// Starts a connection for what isn't saved yet.
    fn dial(&mut self) {
        self.outlet = Arc::new(Outlet::default());
        let dial = Dial {
            id: self.connection,
            url: self.job.url.clone(),
            from: self.progress.received,
        };
        connection::spawn(
            http::agent(),
            dial,
            Arc::clone(&self.outlet),
            self.connect.clone(),
        );
    }

    /// Stops the connection, keeping what it saved, unless already paused.
    fn pause(
        &mut self,
        report: &impl Fn(Report) -> Result<(), Abandoned>,
    ) -> Result<(), Abandoned> {
        if self.paused {
            return Ok(());
        }
        self.paused = true;
        let closed = self.outlet.close();
        if closed.file.is_some() {
            self.progress.received = self.offset + closed.written;
        }
        self.progress.speed = 0;
        report(Report::Paused(self.progress))
    }

    /// Carries on after a pause with a new connection, unless the file was complete, when
    /// it is saved at once.
    fn resume(
        &mut self,
        report: &impl Fn(Report) -> Result<(), Abandoned>,
    ) -> Result<Option<Report>, Stopped> {
        if !self.paused {
            return Ok(None);
        }
        self.paused = false;
        report(Report::Resumed)?;
        if self.partial.is_some() && self.progress.total == Some(self.progress.received) {
            return self.save().map(Some);
        }
        self.meter = SpeedMeter::default();
        self.last_report = None;
        self.connection += 1;
        self.dial();
        Ok(None)
    }

    /// Opens the file for the connection the server answered: a new partial file the
    /// first time, or the one already started, from where the server's body begins.
    fn answered(&mut self, answer: &Answer) -> Result<(), Stopped> {
        let file = match &self.partial {
            None => {
                let folder = expand_home(&self.job.folder)?;
                fs::create_dir_all(&folder)?;
                let (partial, file) = Partial::create(&folder, &file_name(&answer.uri))?;
                self.partial = Some(partial);
                file
            }
            Some(partial) => {
                let saved = self.progress.received;
                if answer.starts_at > saved {
                    return Err(Stopped::Failed(DownloadError::PartTooFar {
                        starts_at: answer.starts_at,
                        saved,
                    }));
                }
                let mut file = OpenOptions::new().write(true).open(partial.path())?;
                file.set_len(answer.starts_at)?;
                file.seek(SeekFrom::Start(answer.starts_at))?;
                file
            }
        };
        self.offset = answer.starts_at;
        self.progress.received = answer.starts_at;
        self.progress.total = answer.total;
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

    /// Makes sure what the connection wrote is on disk, and gives the file its name.
    fn save(&mut self) -> Result<Report, Stopped> {
        if let Some(file) = self.outlet.close().file {
            file.sync_all()?;
        }
        let path = self
            .partial
            .as_ref()
            .expect("a body only ends after its answer created the file")
            .finish()?;
        self.partial = None;
        Ok(Report::Saved {
            path,
            size: self.progress.received,
        })
    }

    /// Stops the connection and removes what it wrote, saying why the download stopped.
    fn clean_up(&mut self, stopped: Stopped) -> Stopped {
        self.outlet.close();
        let Some(partial) = self.partial.take() else {
            return stopped;
        };
        match fs::remove_file(partial.path()) {
            Ok(()) => stopped,
            Err(_) => Stopped::Failed(DownloadError::LeftBehind {
                reason: Box::new(stopped),
                path: partial.path().to_owned(),
            }),
        }
    }
}
