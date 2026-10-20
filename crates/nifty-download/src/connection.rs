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

/// Where a connection writes: nowhere yet, the download's file, or nowhere ever again.
#[derive(Debug, Default)]
pub(crate) struct Outlet {
    slot: Mutex<Slot>,
    changed: Condvar,
}

#[derive(Debug, Default)]
enum Slot {
    #[default]
    Waiting,
    Open(File),
    Closed,
}

impl Outlet {
    /// Lets the connection write into `file`, from its current position on.
    pub(crate) fn open(&self, file: File) {
        *self.lock() = Slot::Open(file);
        self.changed.notify_all();
    }

    /// Stops the connection writing, and gives back the file it was writing, if any. Once
    /// this returns, the connection won't write again.
    pub(crate) fn close(&self) -> Option<File> {
        let closed = std::mem::replace(&mut *self.lock(), Slot::Closed);
        self.changed.notify_all();
        match closed {
            Slot::Open(file) => Some(file),
            Slot::Waiting | Slot::Closed => None,
        }
    }

    /// Writes `bytes` once the outlet opens and tells `events`, both before it can close.
    /// `Ok(false)` once it is closed, or nobody is listening.
    fn write(&self, bytes: &[u8], events: &Sender<Event>) -> io::Result<bool> {
        let mut slot = self.lock();
        while matches!(*slot, Slot::Waiting) {
            slot = self
                .changed
                .wait(slot)
                .expect("nothing panics while holding an outlet");
        }
        let Slot::Open(file) = &mut *slot else {
            return Ok(false);
        };
        file.write_all(bytes)?;
        Ok(events.send(Event::Wrote(bytes.len() as u64)).is_ok())
    }

    fn lock(&self) -> MutexGuard<'_, Slot> {
        self.slot
            .lock()
            .expect("nothing panics while holding an outlet")
    }
}

/// Starts a connection asking `agent` for `url`, which writes into `outlet` and tells
/// `events` how it goes.
pub(crate) fn spawn(agent: Agent, url: String, outlet: Arc<Outlet>, events: Sender<Event>) {
    thread::spawn(move || {
        let ended = match http::get(&agent, &url) {
            Ok((answer, body)) => {
                if events.send(Event::Answered(answer)).is_err() {
                    return;
                }
                copy(body.into_reader(), &outlet, &events)
            }
            Err(error) => Err(error.into()),
        };
        // Once the download has moved on, nobody needs to hear how this one ended.
        if let Some(ended) = ended.transpose() {
            let _ = events.send(Event::Ended(ended));
        }
    });
}

/// Copies `body` through `outlet`: `Some(Ok(()))` at the end of the body, `None` when the
/// download stopped listening first.
fn copy(
    mut body: impl Read,
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
            .write(&buffer[..read], events)
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
        }
    }
}
