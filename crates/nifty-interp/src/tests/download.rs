use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::thread;

use nifty_runtime::testing::FakeKind;

use super::start;

/// Answers one HTTP request with `status` and `body`, returning the server's address.
fn serve_once(status: &'static str, body: &'static str) -> String {
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
fn scratch_folder(name: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!("nf-interp-test-{name}"));
    if folder.exists() {
        fs::remove_dir_all(&folder).expect("remove stale scratch folder");
    }
    folder
}

fn downloader(url: &str, folder: &Path) -> String {
    format!(
        "state percent = 0\nstate status = \"\"\n\
         window(\"Downloader\") {{\n\
           button(\"Get\") {{ download(\"{url}\", to: {}, progress: $percent, status: $status) }}\n\
           label(\"{{percent}}% {{status}}\")\n\
         }}",
        nifty_syntax::string_literal(&folder.display().to_string())
    )
}

#[test]
fn saves_the_file_and_reports_when_done() {
    let folder = scratch_folder("download-saves");
    let url = format!("{}/files/hello.txt", serve_once("200 OK", "hello, nifty"));
    let app = start(&downloader(&url, &folder));

    app.click("Get");
    assert_eq!(
        app.labels(),
        ["0% Downloading…"],
        "the click shows it has begun"
    );

    app.app.wait_for_downloads();
    app.fake.run_pending_wakes();

    let saved = folder.join("hello.txt");
    assert_eq!(app.labels(), [format!("100% Saved to {}", saved.display())]);
    assert_eq!(
        fs::read_to_string(&saved).expect("read the saved file"),
        "hello, nifty"
    );
    assert!(app.errors.borrow().is_empty());
    fs::remove_dir_all(&folder).expect("remove scratch folder");
}

#[test]
fn reports_a_failure_and_leaves_no_file() {
    let folder = scratch_folder("download-fails");
    let url = format!("{}/missing.zip", serve_once("404 Not Found", "nope"));
    let app = start(&downloader(&url, &folder));

    app.click("Get");
    app.app.wait_for_downloads();
    app.fake.run_pending_wakes();

    assert_eq!(app.labels(), ["0% Failed: http status: 404"]);
    assert!(!folder.join("missing.zip").exists());
    assert!(app.errors.borrow().is_empty());
}

#[test]
fn the_downloader_example_tracks_each_download() {
    let folder = scratch_folder("downloader-example");
    let url = format!("{}/report.pdf", serve_once("200 OK", "%PDF"));
    let app = start(include_str!("../../../../examples/downloader/main.nf"));
    let [url_field, folder_field] = app.fake.find(FakeKind::TextField)[..] else {
        panic!("the downloader has an address field and a folder field");
    };

    app.fake.type_text(url_field, &url);
    app.fake
        .type_text(folder_field, &folder.display().to_string());
    app.click("Download");
    app.app.wait_for_downloads();
    app.fake.run_pending_wakes();

    let saved = folder.join("report.pdf");
    assert_eq!(
        app.labels(),
        [
            "Downloader".to_owned(),
            "Save to".to_owned(),
            url,
            "100%".to_owned(),
            format!("Saved to {}", saved.display()),
        ]
    );
    assert_eq!(
        app.fake.text(url_field),
        "",
        "the address field clears for the next one"
    );
    assert!(saved.exists());
    fs::remove_dir_all(&folder).expect("remove scratch folder");
}
