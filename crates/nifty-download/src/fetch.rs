//! `fetch(…)`, shared by the interpreter and compiled apps: each fetch asks for a web page on
//! its own thread and sends what came back over a channel, waking the UI thread, which
//! writes it wherever the app's call bound it and then runs the call's block.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
#[cfg(feature = "testing")]
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use nifty_runtime::{FetchField, Waker};
use ureq::Agent;

use crate::http;
use crate::report::Written;

/// What `status` shows from the call until the response arrives.
const FETCHING: &str = "Fetching…";
/// What `status` shows once a response with a status below `400` arrived.
const FETCHED: &str = "Fetched";
/// The largest body a fetch reads, so a huge page can't exhaust the app's memory.
const MAX_BODY: u64 = 10 * 1000 * 1000;
/// The first HTTP status that says the request failed.
const FIRST_ERROR_CODE: u16 = 400;

/// A fetch an action asked for, started once the action has finished. `targets` says where
/// its response goes and what runs then, in whatever terms the app uses.
#[derive(Debug)]
pub struct FetchRequest<T> {
    pub url: String,
    /// Headers to send, each written `Name: value`.
    pub headers: Vec<String>,
    pub targets: T,
}

/// What a fetch got back.
#[derive(Debug, PartialEq, Eq)]
pub enum Fetched {
    /// The server answered, with any status.
    Answered(Response),
    /// No response came, for this reason, such as a name that doesn't exist.
    Failed(String),
}

/// A server's response.
#[derive(Debug, PartialEq, Eq)]
pub struct Response {
    /// The HTTP status code, such as `200`.
    pub code: u16,
    /// The body, read as UTF-8, with anything that isn't replaced by `�`.
    pub text: String,
    /// Each header as `name: value`, with the name in lower case.
    pub headers: Vec<String>,
}

impl Fetched {
    /// The fields a fetch writes as soon as it is called: it has nothing yet.
    #[must_use]
    pub fn started() -> Vec<(FetchField, Written)> {
        vec![
            (FetchField::Status, Written::Text(FETCHING.to_owned())),
            (FetchField::Text, Written::Text(String::new())),
            (FetchField::Code, Written::Int(0)),
            (FetchField::ResponseHeaders, Written::Texts(Vec::new())),
        ]
    }

    /// The fields this outcome writes, with their values. A field the call didn't bind is
    /// skipped by whoever applies them.
    #[must_use]
    pub fn writes(&self) -> Vec<(FetchField, Written)> {
        match self {
            Self::Answered(response) => {
                let status = if response.code < FIRST_ERROR_CODE {
                    FETCHED.to_owned()
                } else {
                    format!("Failed: http status: {}", response.code)
                };
                vec![
                    (FetchField::Status, Written::Text(status)),
                    (FetchField::Text, Written::Text(response.text.clone())),
                    (FetchField::Code, Written::Int(i64::from(response.code))),
                    (
                        FetchField::ResponseHeaders,
                        Written::Texts(response.headers.clone()),
                    ),
                ]
            }
            Self::Failed(reason) => {
                let mut writes = Self::started();
                writes[0] = (
                    FetchField::Status,
                    Written::Text(format!("Failed: {reason}")),
                );
                writes
            }
        }
    }
}

/// Which fetch a response belongs to, while it waits for one.
type FetchId = u64;

/// The fetches an app is waiting on, and the channel their responses arrive on.
pub struct Fetches<T> {
    waker: Arc<dyn Waker>,
    agent: Agent,
    sender: Sender<(FetchId, Fetched)>,
    receiver: Receiver<(FetchId, Fetched)>,
    waiting: RefCell<HashMap<FetchId, Waiting<T>>>,
    next_id: Cell<FetchId>,
    /// Responses a test waited for, handed out before any others.
    #[cfg(feature = "testing")]
    waited: RefCell<VecDeque<(FetchId, Fetched)>>,
}

struct Waiting<T> {
    targets: T,
    /// Joined once its response arrives; `None` when a test is already waiting on it.
    worker: Option<JoinHandle<()>>,
}

impl<T> Fetches<T> {
    /// `waker` asks the UI thread to call [`Fetches::receive`].
    pub fn new(waker: Arc<dyn Waker>) -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            waker,
            agent: http::lenient_agent(),
            sender,
            receiver,
            waiting: RefCell::new(HashMap::new()),
            next_id: Cell::new(0),
            #[cfg(feature = "testing")]
            waited: RefCell::new(VecDeque::new()),
        }
    }

    pub fn start(&self, request: FetchRequest<T>) {
        let FetchRequest {
            url,
            headers,
            targets,
        } = request;
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        let agent = self.agent.clone();
        let sender = self.sender.clone();
        let waker = Arc::clone(&self.waker);
        let worker = thread::spawn(move || {
            let fetched = fetch(&agent, &url, &headers);
            // Sending fails only once the app is gone, when nobody wants the response.
            if sender.send((id, fetched)).is_ok() {
                waker.wake();
            }
        });
        self.waiting.borrow_mut().insert(
            id,
            Waiting {
                targets,
                worker: Some(worker),
            },
        );
    }

    /// Hands each response that has arrived to `apply` with its targets, which are then
    /// forgotten. Stops at the first error, leaving later responses queued.
    ///
    /// # Errors
    /// The first error `apply` returns.
    ///
    /// # Panics
    /// When a response arrives for a fetch this doesn't know, which can't happen: each is
    /// forgotten only once its response arrives.
    pub fn receive<E>(&self, mut apply: impl FnMut(T, &Fetched) -> Result<(), E>) -> Result<(), E> {
        while let Some((id, fetched)) = self.next_response() {
            let waiting = self
                .waiting
                .borrow_mut()
                .remove(&id)
                .expect("responses only come from fetches still waiting");
            if let Some(worker) = waiting.worker {
                join(worker);
            }
            apply(waiting.targets, &fetched)?;
        }
        Ok(())
    }

    #[cfg(not(feature = "testing"))]
    fn next_response(&self) -> Option<(FetchId, Fetched)> {
        self.receiver.try_recv().ok()
    }

    #[cfg(feature = "testing")]
    fn next_response(&self) -> Option<(FetchId, Fetched)> {
        let waited = self.waited.borrow_mut().pop_front();
        waited.or_else(|| self.receiver.try_recv().ok())
    }

    /// Waits until every fetch's response has arrived, keeps them for
    /// [`Fetches::receive`], and wakes the UI thread to receive them.
    ///
    /// # Panics
    /// Never: this holds a sender, so receiving can't disconnect.
    #[cfg(feature = "testing")]
    pub fn wait(&self) {
        let workers: Vec<_> = self
            .waiting
            .borrow_mut()
            .values_mut()
            .filter_map(|waiting| waiting.worker.take())
            .collect();
        for worker in workers {
            join(worker);
            let response = self
                .receiver
                .recv()
                .expect("this holds a sender, so receiving never disconnects");
            self.waited.borrow_mut().push_back(response);
        }
        self.waker.wake();
    }
}

fn join(worker: JoinHandle<()>) {
    worker.join().expect("fetch workers don't panic");
}

/// Asks for `url` with `headers`, and reads the whole response.
fn fetch(agent: &Agent, url: &str, headers: &[String]) -> Fetched {
    let headers = match http::headers(headers) {
        Ok(headers) => headers,
        Err(bad) => return Fetched::Failed(bad.to_string()),
    };
    let mut request = agent.get(url);
    for (name, value) in headers.iter() {
        request = request.header(name, value);
    }
    let mut response = match request.call() {
        Ok(response) => response,
        Err(error) => return Fetched::Failed(error.to_string()),
    };
    let code = response.status().as_u16();
    let headers = response
        .headers()
        .iter()
        .map(|(name, value)| format!("{name}: {}", String::from_utf8_lossy(value.as_bytes())))
        .collect();
    match response
        .body_mut()
        .with_config()
        .limit(MAX_BODY)
        .read_to_vec()
    {
        Ok(body) => Fetched::Answered(Response {
            code,
            text: String::from_utf8_lossy(&body).into_owned(),
            headers,
        }),
        Err(error) => Fetched::Failed(error.to_string()),
    }
}

impl<T> std::fmt::Debug for Fetches<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Fetches")
            .field("waiting", &self.waiting.borrow().len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(code: u16) -> Fetched {
        Fetched::Answered(Response {
            code,
            text: "body".to_owned(),
            headers: vec!["set-cookie: a=1".to_owned()],
        })
    }

    #[test]
    fn an_answer_writes_everything_it_got() {
        assert_eq!(
            response(200).writes(),
            [
                (FetchField::Status, Written::Text("Fetched".to_owned())),
                (FetchField::Text, Written::Text("body".to_owned())),
                (FetchField::Code, Written::Int(200)),
                (
                    FetchField::ResponseHeaders,
                    Written::Texts(vec!["set-cookie: a=1".to_owned()])
                ),
            ]
        );
    }

    #[test]
    fn an_error_status_fails_but_keeps_what_the_server_said() {
        let writes = response(404).writes();
        assert_eq!(
            writes[0],
            (
                FetchField::Status,
                Written::Text("Failed: http status: 404".to_owned())
            )
        );
        assert_eq!(
            writes[1],
            (FetchField::Text, Written::Text("body".to_owned()))
        );
    }

    #[test]
    fn no_response_clears_everything_but_the_reason() {
        let writes = Fetched::Failed("host not found".to_owned()).writes();
        assert_eq!(
            writes[0],
            (
                FetchField::Status,
                Written::Text("Failed: host not found".to_owned())
            )
        );
        assert_eq!(writes[1..], Fetched::started()[1..]);
    }
}
