//! Helpers for tests that run real downloads against this machine.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::thread;

/// Answers one HTTP request with `status` and `body`, returning the server's address.
///
/// # Panics
/// When no local port can be bound, or the request can't be read or answered.
#[must_use]
pub fn serve_once(status: &'static str, body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a local port");
    let address = listener.local_addr().expect("a bound port has an address");
    thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept the download's request");
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        while reader.read_line(&mut line).expect("read the request") > 2 {
            line.clear();
        }
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        reader
            .get_mut()
            .write_all(response.as_bytes())
            .expect("send the response");
    });
    format!("http://{address}")
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
