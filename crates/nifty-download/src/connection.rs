//! One connection to the server, on its own thread: it asks for the file and writes the
//! body into the file the download gives it, telling the download each step.
//!
//! A connection blocked waiting for the server can't be interrupted, so the download
//! never waits for one: it closes the connection's [`Outlet`], and the connection stops at
//! its next write without touching the file again.

use std::fs::File;
use std::io::{self, Read, Write};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;

use ureq::Agent;

use crate::http;
use crate::worker::Event;

const CHUNK_SIZE: usize = 64 * 1024;

/// Which of a download's connections an event comes from, counted from 0, so events from
/// one it has stopped can be told apart from the one it runs now.
pub(crate) type ConnectionId = usize;

/// Where a connection writes: nowhere yet, the download's file, or nowhere ever again.
#[derive(Debug, Default)]
pub(crate) struct Outlet {
    inner: Mutex<Inner>,
    changed: Condvar,
}

#[derive(Debug, Default)]
struct Inner {
    slot: Slot,
    /// Bytes written through it so far.
    written: u64,
}

#[derive(Debug, Default)]
enum Slot {
    #[default]
    Waiting,
    Open(File),
    Closed,
}

/// What a closed outlet leaves: the file it was writing, if it had opened, and how many
/// bytes went through it.
#[derive(Debug)]
pub(crate) struct Closed {
    pub file: Option<File>,
    pub written: u64,
}

impl Outlet {
    /// Lets the connection write into `file`, from its current position on.
    pub(crate) fn open(&self, file: File) {
        self.lock().slot = Slot::Open(file);
        self.changed.notify_all();
    }

    /// Stops the connection writing. Once this returns, the connection won't write again.
    pub(crate) fn close(&self) -> Closed {
        let mut inner = self.lock();
        let slot = std::mem::replace(&mut inner.slot, Slot::Closed);
        let written = inner.written;
        drop(inner);
        self.changed.notify_all();
        let file = match slot {
            Slot::Open(file) => Some(file),
            Slot::Waiting | Slot::Closed => None,
        };
        Closed { file, written }
    }

    /// Writes `bytes` once the outlet opens and tells `events`, both before it can close.
    /// `Ok(false)` once it is closed, or nobody is listening.
    fn write(&self, bytes: &[u8], id: ConnectionId, events: &Sender<Event>) -> io::Result<bool> {
        let mut inner = self.lock();
        while matches!(inner.slot, Slot::Waiting) {
            inner = self
                .changed
                .wait(inner)
                .expect("nothing panics while holding an outlet");
        }
        let Slot::Open(file) = &mut inner.slot else {
            return Ok(false);
        };
        file.write_all(bytes)?;
        let count = bytes.len() as u64;
        inner.written += count;
        Ok(events.send(Event::Wrote(id, count)).is_ok())
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .expect("nothing panics while holding an outlet")
    }
}

/// What a connection asks for: `url`, from byte `from` on.
#[derive(Debug)]
pub(crate) struct Dial {
    pub id: ConnectionId,
    pub url: String,
    pub from: u64,
}

/// Starts a connection asking `agent` for what `dial` says, which writes into `outlet`
/// and tells `events` how it goes.
pub(crate) fn spawn(agent: Agent, dial: Dial, outlet: Arc<Outlet>, events: Sender<Event>) {
    thread::spawn(move || {
        let id = dial.id;
        let ended = match http::get(&agent, &dial.url, dial.from) {
            Ok((answer, body)) => {
                if events.send(Event::Answered(id, answer)).is_err() {
                    return;
                }
                copy(body.into_reader(), id, &outlet, &events)
            }
            Err(error) => Err(error),
        };
        // Once the download has moved on, nobody needs to hear how this one ended.
        if let Some(ended) = ended.transpose() {
            let _ = events.send(Event::Ended(id, ended));
        }
    });
}

/// Copies `body` through `outlet`: `Some(Ok(()))` at the end of the body, `None` when the
/// download stopped listening first.
fn copy(
    mut body: impl Read,
    id: ConnectionId,
    outlet: &Outlet,
    events: &Sender<Event>,
) -> Result<Option<()>, ConnectionError> {
    let mut buffer = vec![0; CHUNK_SIZE];
    loop {
        let read = body.read(&mut buffer).map_err(ConnectionError::Read)?;
        if read == 0 {
            return Ok(Some(()));
        }
        if !outlet
            .write(&buffer[..read], id, events)
            .map_err(ConnectionError::Write)?
        {
            return Ok(None);
        }
    }
}

/// Why a connection stopped early.
#[derive(Debug)]
pub(crate) enum ConnectionError {
    Http(ureq::Error),
    /// Reading the body from the server failed.
    Read(io::Error),
    /// Writing the file failed.
    Write(io::Error),
    /// The server sent part of the file without saying which part.
    UnclearPart,
}

impl From<ureq::Error> for ConnectionError {
    fn from(error: ureq::Error) -> Self {
        Self::Http(error)
    }
}

impl std::fmt::Display for ConnectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http(error) => write!(f, "{error}"),
            Self::Read(error) | Self::Write(error) => write!(f, "{error}"),
            Self::UnclearPart => {
                f.write_str("the server sent part of the file without saying which")
            }
        }
    }
}
