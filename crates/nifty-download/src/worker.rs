//! A download's own thread, which handles everything that happens to it, in order: its
//! connections' progress, and what the app asks of it.
//!
//! A download asked to use several connections asks for the whole file as a range. A
//! server that answers with part of it and the file's size gets the file split into
//! parts, each fetched by its own connection into its own place in the partial file: the
//! first connection keeps the first part, and new ones ask for the rest. Any other answer
//! leaves one connection fetching the whole file.
//!
//! Pausing closes each connection's outlet and keeps the partial file; resuming opens a
//! new connection for each part not yet saved, asking for the rest of it with an HTTP
//! range request. A server that ignores the range sends the whole file again, so a
//! download with one connection starts over from its first byte.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use nifty_runtime::{DownloadCommand, NoHomeFolder, expand_home};

use ureq::Agent;

use crate::connection::{self, ConnectionError, ConnectionId, Dial, Outlet};
use crate::http::{self, Answer};
use crate::name::{Partial, file_name, safe_file_name};
use crate::part::{self, Live, Part};
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

/// What a download fetches, where it saves it, and how many connections it may open.
#[derive(Debug)]
pub(crate) struct Job {
    pub url: String,
    pub folder: String,
    /// The file's name, made safe before use; `None` names it after the address.
    pub name: Option<String>,
    pub connections: usize,
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
    /// The server sent a part of the file other than the one a connection asked for.
    PartMoved {
        starts_at: u64,
        asked: u64,
    },
    /// A connection's body ended before the end of its part.
    PartCutShort {
        saved: u64,
        end: u64,
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
            Self::PartMoved { starts_at, asked } => write!(
                f,
                "the server sent the file from byte {starts_at} when asked for byte {asked} on"
            ),
            Self::PartCutShort { saved, end } => write!(
                f,
                "the server stopped sending the file at byte {saved}, before byte {end}"
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
        agent: http::agent(),
        connect,
        next_connection: 0,
        parts: vec![Part::whole()],
        partial: None,
        total: None,
        speed: 0,
        paused: false,
        meter: SpeedMeter::default(),
        last_report: None,
    };
    download.dial(0);
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
    agent: Agent,
    connect: Sender<Event>,
    /// The id the next connection gets.
    next_connection: ConnectionId,
    /// In the order they come in the file, which they cover between them.
    parts: Vec<Part>,
    /// The file being written, once a server has answered.
    partial: Option<Partial>,
    /// The whole file's length, when the server said.
    total: Option<u64>,
    speed: u64,
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
                // A connection stopped by a pause or a cancel may still be heard from, but
                // it no longer fetches a part.
                Event::Answered(id, answer) => {
                    if let Some(index) = self.part_of(id) {
                        self.answered(index, &answer)?;
                    }
                }
                Event::Wrote(id, bytes) => {
                    if let Some(index) = self.part_of(id) {
                        self.wrote(index, bytes, report)?;
                    }
                }
                Event::Ended(id, ended) => {
                    if let Some(index) = self.part_of(id)
                        && let Some(saved) = self.ended(index, ended)?
                    {
                        return Ok(saved);
                    }
                }
            }
        }
    }

    /// Which part connection `id` is fetching, unless it has been stopped.
    fn part_of(&self, id: ConnectionId) -> Option<usize> {
        self.parts.iter().position(|part| {
            part.connection
                .as_ref()
                .is_some_and(|connection| connection.id == id)
        })
    }

    /// Starts a connection for what isn't saved yet of the part at `index`. The whole
    /// file is asked for as a range while the download may still split it, so the answer
    /// says whether the server can send parts.
    fn dial(&mut self, index: usize) {
        let id = self.next_connection;
        self.next_connection += 1;
        let rest = self.parts[index].rest();
        let may_split = self.job.connections > 1 && self.partial.is_none();
        let whole = rest.from == 0 && rest.to.is_none();
        let outlet = Arc::new(Outlet::default());
        self.parts[index].connection = Some(Live {
            id,
            outlet: Arc::clone(&outlet),
            writes_from: None,
        });
        let dial = Dial {
            id,
            url: self.job.url.clone(),
            range: (may_split || !whole).then_some(rest),
        };
        connection::spawn(self.agent.clone(), dial, outlet, self.connect.clone());
    }

    fn progress(&self) -> Progress {
        Progress {
            received: self.parts.iter().map(|part| part.saved).sum(),
            total: self.total,
            speed: self.speed,
        }
    }

    /// Stops every connection, keeping what they saved, unless already paused.
    fn pause(
        &mut self,
        report: &impl Fn(Report) -> Result<(), Abandoned>,
    ) -> Result<(), Abandoned> {
        if self.paused {
            return Ok(());
        }
        self.paused = true;
        for part in &mut self.parts {
            part.hang_up();
        }
        self.speed = 0;
        report(Report::Paused(self.progress()))
    }

    /// Carries on after a pause with a new connection for each part not yet saved, unless
    /// the file was complete, when it is saved at once.
    fn resume(
        &mut self,
        report: &impl Fn(Report) -> Result<(), Abandoned>,
    ) -> Result<Option<Report>, Stopped> {
        if !self.paused {
            return Ok(None);
        }
        self.paused = false;
        report(Report::Resumed)?;
        if self.partial.is_some() && self.is_complete() {
            return self.save().map(Some);
        }
        self.meter = SpeedMeter::default();
        self.last_report = None;
        for index in 0..self.parts.len() {
            if !self.parts[index].is_complete(self.total) {
                self.dial(index);
            }
        }
        Ok(None)
    }

    fn is_complete(&self) -> bool {
        self.parts.iter().all(|part| part.is_complete(self.total))
    }

    /// Opens the file for the connection the server answered, from where its body begins:
    /// a new partial file the first time, when the file may also be split into parts.
    fn answered(&mut self, index: usize, answer: &Answer) -> Result<(), Stopped> {
        let created = match self.partial {
            Some(_) => None,
            None => Some(self.create(answer)?),
        };
        let asked = self.parts[index].position();
        // Alone, a part can start over from an earlier byte; beside others, it can't.
        let alone = self.parts.len() == 1;
        if alone && answer.starts_at > asked {
            return Err(Stopped::Failed(DownloadError::PartTooFar {
                starts_at: answer.starts_at,
                saved: asked,
            }));
        }
        if !alone && answer.starts_at != asked {
            return Err(Stopped::Failed(DownloadError::PartMoved {
                starts_at: answer.starts_at,
                asked,
            }));
        }
        let mut file = match created {
            Some(file) => file,
            None => OpenOptions::new().write(true).open(self.partial_path())?,
        };
        if alone {
            self.total = answer.total;
            file.set_len(answer.starts_at)?;
        }
        file.seek(SeekFrom::Start(answer.starts_at))?;
        let part = &mut self.parts[index];
        part.saved = answer.starts_at - part.start;
        let room = part.room();
        let live = part
            .connection
            .as_mut()
            .expect("only a live connection's answer is handled");
        live.writes_from = Some(answer.starts_at);
        live.outlet.open(file, room);
        Ok(())
    }

    /// Creates the partial file for the first answer, and splits the file into parts when
    /// the download may and the server can send them, starting a connection for each part
    /// after the first.
    fn create(&mut self, answer: &Answer) -> Result<File, Stopped> {
        let folder = expand_home(&self.job.folder)?;
        fs::create_dir_all(&folder)?;
        let name = self
            .job
            .name
            .as_deref()
            .map_or_else(|| file_name(&answer.uri), safe_file_name);
        let (partial, file) = Partial::create(&folder, &name)?;
        self.partial = Some(partial);
        if self.job.connections > 1
            && answer.partial
            && let Some(total) = answer.total
        {
            self.total = Some(total);
            let mut parts = part::split(total, self.job.connections);
            parts[0].connection = self.parts[0].connection.take();
            self.parts = parts;
            for index in 1..self.parts.len() {
                self.dial(index);
            }
        }
        Ok(file)
    }

    fn partial_path(&self) -> &Path {
        self.partial
            .as_ref()
            .expect("a connection's file is opened after the first answer created it")
            .path()
    }

    fn wrote(
        &mut self,
        index: usize,
        bytes: u64,
        report: &impl Fn(Report) -> Result<(), Abandoned>,
    ) -> Result<(), Abandoned> {
        self.parts[index].saved += bytes;
        let now = Instant::now();
        self.speed = self.meter.record(now, self.progress().received);
        if self
            .last_report
            .is_none_or(|last| now.duration_since(last) >= REPORT_INTERVAL)
        {
            report(Report::Progress(self.progress()))?;
            self.last_report = Some(now);
        }
        Ok(())
    }

    /// Handles the end of the part at `index`'s connection: makes sure what it wrote is
    /// on disk, and saves the file once every part is, giving the report that says so.
    fn ended(
        &mut self,
        index: usize,
        ended: Result<(), ConnectionError>,
    ) -> Result<Option<Report>, Stopped> {
        ended.map_err(|error| Stopped::Failed(DownloadError::Connection(error)))?;
        let part = &mut self.parts[index];
        if let Some(file) = part.hang_up() {
            file.sync_all()?;
        }
        let saved = part.position();
        match part.end.or(self.total) {
            Some(end) if saved < end => {
                return Err(Stopped::Failed(DownloadError::PartCutShort { saved, end }));
            }
            // A body that ends says how long the file is, when the server didn't.
            None => self.total = Some(saved),
            Some(_) => {}
        }
        if self.is_complete() {
            self.save().map(Some)
        } else {
            Ok(None)
        }
    }

    /// Gives the file its name.
    fn save(&mut self) -> Result<Report, Stopped> {
        let path = self
            .partial
            .take()
            .expect("a body only ends after its answer created the file")
            .finish()?;
        Ok(Report::Saved {
            path,
            size: self.progress().received,
        })
    }

    /// Stops every connection and removes what they wrote, saying why the download
    /// stopped.
    fn clean_up(&mut self, stopped: Stopped) -> Stopped {
        for part in &mut self.parts {
            part.hang_up();
        }
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
