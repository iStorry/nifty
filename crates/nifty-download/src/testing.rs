//! Helpers for tests that run real downloads against this machine.

use std::collections::VecDeque;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;

/// The smallest part a download gives a connection of its own, so a test knows how large a
/// file must be to be split.
pub const MIN_PART_SIZE: u64 = crate::part::MIN_PART_SIZE;

/// A body of `len` bytes, each different from its neighbours, so a part written in the
/// wrong place shows.
///
/// # Panics
/// Never: each byte is a remainder below 251.
#[must_use]
pub fn numbered_body(len: u64) -> Vec<u8> {
    (0..len)
        .map(|index| u8::try_from(index % 251).expect("a remainder of 251 fits a byte"))
        .collect()
}

/// A file a [`Server`] serves at every path, and how it describes it.
#[derive(Clone, Debug)]
pub struct Resource {
    pub body: Vec<u8>,
    /// Whether responses say how long the body is. Without it the body ends when the
    /// connection closes, as some servers send it.
    pub length: bool,
    /// Whether a request for part of the file, `Range: bytes=N-` or `Range: bytes=N-M`,
    /// gets just that part, as `206 Partial Content`. Without it, every request gets it all.
    pub ranges: bool,
    /// Headers sent with every response, each written `Name: value`.
    pub headers: Vec<String>,
}

impl Resource {
    #[must_use]
    pub fn new(body: impl Into<Vec<u8>>) -> Self {
        Self {
            body: body.into(),
            length: true,
            ranges: true,
            headers: Vec::new(),
        }
    }

    /// Sends `header`, written `Name: value`, with every response too.
    #[must_use]
    pub fn with_header(mut self, header: &str) -> Self {
        self.headers.push(header.to_owned());
        self
    }

    #[must_use]
    pub fn without_ranges(self) -> Self {
        Self {
            ranges: false,
            ..self
        }
    }

    #[must_use]
    pub fn without_length(self) -> Self {
        Self {
            length: false,
            ..self
        }
    }
}

/// How the server answers one request. Requests take the steps in order; once they run
/// out, each gets the whole resource.
#[derive(Debug)]
pub enum Step {
    /// The whole resource, as `200 OK`.
    Serve,
    /// This status, such as `"404 Not Found"`, with the resource as its body.
    Reply(&'static str),
    /// The resource as `200 OK`, stopping after `after` bytes until the test says.
    Stall {
        after: usize,
        sent: Sender<()>,
        resume: Receiver<()>,
    },
}

/// The test's side of a [`Step::Stall`].
#[derive(Debug)]
pub struct Stalled {
    sent: Receiver<()>,
    resume: Sender<()>,
}

impl Step {
    /// A step that sends the first `after` bytes, then waits for the test.
    #[must_use]
    pub fn stall(after: usize) -> (Self, Stalled) {
        let (sent, sent_inbox) = mpsc::channel();
        let (resume, resume_inbox) = mpsc::channel();
        let step = Self::Stall {
            after,
            sent,
            resume: resume_inbox,
        };
        (
            step,
            Stalled {
                sent: sent_inbox,
                resume,
            },
        )
    }
}

impl Stalled {
    /// Waits until the server has sent the bytes before the stall.
    ///
    /// # Panics
    /// When the server never reached the stall.
    pub fn wait_until_sent(&self) {
        self.sent.recv().expect("the server reaches the stall");
    }

    /// Lets the server send the rest. Dropping this instead closes the connection there.
    ///
    /// # Panics
    /// When the server is no longer waiting.
    pub fn resume(&self) {
        self.resume.send(()).expect("the server waits at the stall");
    }
}

/// A request the server received.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seen {
    pub path: String,
    /// Each header as `name: value`, with the name in lower case.
    pub headers: Vec<String>,
}

/// An HTTP server on a local port, answering each connection on its own thread until the
/// test process ends.
#[derive(Debug)]
pub struct Server {
    address: String,
    seen: Arc<Mutex<Vec<Seen>>>,
}

impl Server {
    /// Serves `resource`, answering requests as `steps` say.
    ///
    /// # Panics
    /// When no local port can be bound.
    #[must_use]
    pub fn start(resource: Resource, steps: impl IntoIterator<Item = Step>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a local port");
        let address = format!(
            "http://{}",
            listener.local_addr().expect("a bound port has an address")
        );
        let seen = Arc::new(Mutex::new(Vec::new()));
        let steps = Arc::new(Mutex::new(steps.into_iter().collect::<VecDeque<_>>()));
        let log = Arc::clone(&seen);
        thread::spawn(move || {
            for stream in listener.incoming() {
                let stream = stream.expect("accept a download's connection");
                let resource = resource.clone();
                let (steps, log) = (Arc::clone(&steps), Arc::clone(&log));
                thread::spawn(move || answer(stream, &resource, &steps, &log));
            }
        });
        Self { address, seen }
    }

    /// The address of `path` on this server, such as `/files/a.zip`.
    #[must_use]
    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.address)
    }

    /// The requests received so far, in the order they arrived.
    #[must_use]
    pub fn seen(&self) -> Vec<Seen> {
        lock(&self.seen).clone()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    // A panicking connection thread fails its test anyway; what it logged is still useful.
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn answer(
    stream: TcpStream,
    resource: &Resource,
    steps: &Mutex<VecDeque<Step>>,
    log: &Mutex<Vec<Seen>>,
) {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).expect("read the request line");
    let path = line
        .split_whitespace()
        .nth(1)
        .unwrap_or_default()
        .to_owned();
    let mut headers = Vec::new();
    loop {
        line.clear();
        reader.read_line(&mut line).expect("read a request header");
        let header = line.trim_end();
        if header.is_empty() {
            break;
        }
        let (name, value) = header.split_once(':').unwrap_or((header, ""));
        headers.push(format!("{}: {}", name.to_lowercase(), value.trim()));
    }
    let whole = resource.body.len();
    let range = headers
        .iter()
        .find_map(|header| header.strip_prefix("range: bytes="))
        .and_then(|range| asked_range(range, whole))
        .filter(|_| resource.ranges);
    lock(log).push(Seen { path, headers });
    let step = lock(steps).pop_front().unwrap_or(Step::Serve);
    let (status, body, range) = match (&step, range) {
        (Step::Reply(status), _) => (*status, &resource.body[..], String::new()),
        (Step::Serve | Step::Stall { .. }, Some((from, to))) => (
            "206 Partial Content",
            &resource.body[from..to],
            format!("Content-Range: bytes {from}-{}/{whole}\r\n", to - 1),
        ),
        (Step::Serve | Step::Stall { .. }, None) => ("200 OK", &resource.body[..], String::new()),
    };
    let length = if resource.length {
        format!("Content-Length: {}\r\n", body.len())
    } else {
        String::new()
    };
    let extra: String = resource
        .headers
        .iter()
        .flat_map(|header| [header.as_str(), "\r\n"])
        .collect();
    let head = format!("HTTP/1.1 {status}\r\nConnection: close\r\n{range}{length}{extra}\r\n");
    let stream = reader.get_mut();
    stream
        .write_all(head.as_bytes())
        .expect("send the response's head");
    if let Step::Stall {
        after,
        sent,
        resume,
    } = step
    {
        // A download that stops listening closes the connection, which can fail a write.
        if stream
            .write_all(&body[..after])
            .and_then(|()| stream.flush())
            .is_err()
        {
            return;
        }
        sent.send(()).expect("the test waits for the stall");
        if resume.recv().is_err() {
            return;
        }
        let _ = stream.write_all(&body[after..]);
        return;
    }
    let _ = stream.write_all(body);
}

/// The bytes from the first up to, but not including, the second that a `Range` such as
/// `0-99` or `100-` asks for, of a body `whole` bytes long.
fn asked_range(range: &str, whole: usize) -> Option<(usize, usize)> {
    let (from, last) = range.split_once('-')?;
    let from = from.parse().ok()?;
    let to = if last.is_empty() {
        whole
    } else {
        last.parse::<usize>().ok()?.saturating_add(1).min(whole)
    };
    (from < to).then_some((from, to))
}

/// Answers one HTTP request with `status` and `body`, returning the server's address.
///
/// # Panics
/// When no local port can be bound.
#[must_use]
pub fn serve_once(status: &'static str, body: &'static str) -> String {
    Server::start(Resource::new(body), [Step::Reply(status)]).url("")
}

/// An empty folder for one test, named after it so tests running in parallel don't collide.
///
/// # Panics
/// When a folder left by an earlier run can't be removed.
#[must_use]
pub fn scratch_folder(name: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!("nf-test-{name}"));
    if folder.exists() {
        fs::remove_dir_all(&folder).expect("remove stale scratch folder");
    }
    folder
}
