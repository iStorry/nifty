//! Helpers for tests that run real downloads against this machine.

use std::collections::VecDeque;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;

/// A file a [`Server`] serves at every path, and how it describes it.
#[derive(Clone, Debug)]
pub struct Resource {
    pub body: Vec<u8>,
    /// Whether responses say how long the body is. Without it the body ends when the
    /// connection closes, as some servers send it.
    pub length: bool,
}

impl Resource {
    #[must_use]
    pub fn new(body: impl Into<Vec<u8>>) -> Self {
        Self {
            body: body.into(),
            length: true,
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
    lock(log).push(Seen { path, headers });
    let step = lock(steps).pop_front().unwrap_or(Step::Serve);
    let status = match step {
        Step::Serve => "200 OK",
        Step::Reply(status) => status,
    };
    let length = if resource.length {
        format!("Content-Length: {}\r\n", resource.body.len())
    } else {
        String::new()
    };
    let head = format!("HTTP/1.1 {status}\r\nConnection: close\r\n{length}\r\n");
    let stream = reader.get_mut();
    stream
        .write_all(head.as_bytes())
        .and_then(|()| stream.write_all(&resource.body))
        .expect("send the response");
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
