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
//!
//! A connection that fails in a way that may pass is tried again the same way, after a
//! wait the thread spends listening for events, so the app can pause or cancel meanwhile.

use std::fs::{self, File, OpenOptions};
use std::io::{Seek, SeekFrom};
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use nifty_runtime::{DownloadCommand, expand_home};

use ureq::Agent;

use crate::connection::{self, ConnectionError, ConnectionId, Dial, Outlet};
use crate::http::{self, Answer, Headers};
use crate::name::{Partial, file_name, safe_file_name};
use crate::part::{self, Live, Part};
use crate::report::{Progress, Report};
use crate::retry;
use crate::speed::SpeedMeter;

mod stopped;

use stopped::{DownloadError, Stopped};

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
    /// Headers to send with each request, each written `Name: value`.
    pub headers: Vec<String>,
    /// How many times in a row a connection is tried again after a failure that may pass.
    pub retries: u32,
}

/// The UI thread is gone, so nobody is waiting for this download any more.
#[derive(Debug)]
pub(crate) struct Abandoned;

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
    let headers = match http::headers(&job.headers) {
        Ok(headers) => headers,
        Err(error) => return report(Report::Failed(DownloadError::Header(error).to_string())),
    };
    let mut download = Download {
        job,
        headers,
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
    headers: Headers,
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
            match self.next_event(events, report)? {
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
                        && let Some(saved) = self.ended(index, ended, report)?
                    {
                        return Ok(saved);
                    }
                }
            }
        }
    }

    /// Waits for the next event, trying again each part whose wait to retry ends first.
    fn next_event(
        &mut self,
        events: &Receiver<Event>,
        report: &impl Fn(Report) -> Result<(), Abandoned>,
    ) -> Result<Event, Stopped> {
        loop {
            let next_retry = self.parts.iter().filter_map(|part| part.retry_at).min();
            // The app holds a sender until it lets go of the download.
            let Some(retry_at) = next_retry else {
                return events.recv().map_err(|_| Stopped::Abandoned);
            };
            match events.recv_timeout(retry_at.saturating_duration_since(Instant::now())) {
                Ok(event) => return Ok(event),
                Err(RecvTimeoutError::Timeout) => self.retry_due(report)?,
                Err(RecvTimeoutError::Disconnected) => return Err(Stopped::Abandoned),
            }
        }
    }

    /// Starts a connection again for each part whose wait to retry is over, saying the
    /// download is under way again if nothing was.
    fn retry_due(
        &mut self,
        report: &impl Fn(Report) -> Result<(), Abandoned>,
    ) -> Result<(), Abandoned> {
        let was_waiting = self.is_waiting();
        let now = Instant::now();
        for index in 0..self.parts.len() {
            if self.parts[index].retry_at.is_some_and(|at| at <= now) {
                self.parts[index].retry_at = None;
                self.dial(index);
            }
        }
        if was_waiting {
            report(Report::Downloading)?;
        }
        Ok(())
    }

    /// Whether no connection is running, only parts waiting to retry.
    fn is_waiting(&self) -> bool {
        self.parts.iter().all(|part| part.connection.is_none())
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
            headers: Arc::clone(&self.headers),
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
            part.retry_at = None;
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
        report(Report::Downloading)?;
        if self.partial.is_some() && self.is_complete() {
            return self.save().map(Some);
        }
        self.meter = SpeedMeter::default();
        self.last_report = None;
        for index in 0..self.parts.len() {
            self.parts[index].failures = 0;
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
        let part = &mut self.parts[index];
        part.saved += bytes;
        part.failures = 0;
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
    /// on disk, and saves the file once every part is, giving the report that says so. A
    /// failure that may pass is tried again later while retries are left.
    fn ended(
        &mut self,
        index: usize,
        ended: Result<(), ConnectionError>,
        report: &impl Fn(Report) -> Result<(), Abandoned>,
    ) -> Result<Option<Report>, Stopped> {
        if let Err(error) = ended {
            self.retry_later(index, error, report)?;
            return Ok(None);
        }
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

    /// Stops the part at `index`'s failed connection, keeping what it saved, and tries it
    /// again after a wait, saying so when nothing else is downloading.
    ///
    /// # Errors
    /// The failure, when it can't pass or no retries are left.
    fn retry_later(
        &mut self,
        index: usize,
        error: ConnectionError,
        report: &impl Fn(Report) -> Result<(), Abandoned>,
    ) -> Result<(), Stopped> {
        let part = &mut self.parts[index];
        if !error.is_transient() || part.failures >= self.job.retries {
            return Err(Stopped::Failed(DownloadError::Connection(error)));
        }
        part.hang_up();
        part.failures += 1;
        part.retry_at = Some(Instant::now() + retry::delay(part.failures));
        if self.is_waiting() {
            self.speed = 0;
            report(Report::Retrying)?;
        }
        Ok(())
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
