use std::fs;
use std::path::Path;

use nifty_download::testing::{
    MIN_PART_SIZE, Resource, Server, Stalled, Step, numbered_body, scratch_folder, serve_once,
};
use nifty_runtime::testing::FakeKind;

use super::{Running, start};

fn downloader(url: &str, folder: &Path) -> String {
    downloader_with(url, folder, "")
}

/// Downloads `url` into `folder` with `settings` added to the call, such as
/// `", connections: 4"`.
fn downloader_with(url: &str, folder: &Path, settings: &str) -> String {
    format!(
        "state percent = 0\nstate status = \"\"\n\
         window(\"Downloader\") {{\n\
           button(\"Get\") {{ download(\"{url}\", to: {}, progress: $percent, status: $status{settings}) }}\n\
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
    let app = start(include_str!("../../../../fixtures/downloader.nf"));
    assert_eq!(app.fake.window_count(), 1, "settings wait to be opened");
    let url_field = app.only(FakeKind::TextField);
    app.fake.choose_menu_item(&["Downloader", "Settings…"]);
    let fields = app.fake.find(FakeKind::TextField);
    let Some(&folder_field) = fields.iter().find(|&&field| field != url_field) else {
        panic!("the settings have a folder field");
    };
    app.fake
        .type_text(folder_field, &folder.display().to_string());
    app.click("Done");
    assert!(app.fake.is_closed(1), "Done closes the settings");

    app.fake.type_text(url_field, &url);
    app.click("Download");
    app.app.wait_for_downloads();
    app.fake.run_pending_wakes();

    let saved = folder.join("report.pdf");
    assert_eq!(
        app.labels(),
        [
            "Downloader".to_owned(),
            url,
            "100%".to_owned(),
            format!("Saved to {}", saved.display()),
            "4 bytes of 4 bytes".to_owned(),
        ]
    );
    let bar = app.only(FakeKind::Progress);
    assert_eq!(app.fake.value(bar), Some(100));
    assert_eq!(
        app.fake.text(url_field),
        "",
        "the address field clears for the next one"
    );
    assert!(saved.exists());
    fs::remove_dir_all(&folder).expect("remove scratch folder");
}

/// Binds every report field, showing each in a label with its size in words.
fn detailed_downloader(url: &str, folder: &Path) -> String {
    format!(
        "state percent = 0\nstate status = \"\"\nstate received = 0\nstate total = 0\nstate speed = 0\n\
         window(\"Downloader\") {{\n\
           button(\"Get\") {{ download(\"{url}\", to: {}, progress: $percent, status: $status, received: $received, total: $total, speed: $speed) }}\n\
           label(\"{{percent}}% of {{format_bytes(total)}}\")\n\
           label(\"{{received}} of {{total}} at {{speed}}\")\n\
         }}",
        nifty_syntax::string_literal(&folder.display().to_string())
    )
}

#[test]
fn reports_bytes_received_and_the_size() {
    let folder = scratch_folder("download-bytes");
    let body = "x".repeat(1_500);
    let server = Server::start(Resource::new(body.as_str()), []);
    let app = start(&detailed_downloader(&server.url("/a.bin"), &folder));

    app.click("Get");
    assert_eq!(app.labels(), ["0% of 0 bytes", "0 of 0 at 0"]);
    app.app.wait_for_downloads();
    app.fake.run_pending_wakes();

    assert_eq!(app.labels(), ["100% of 1.5 KB", "1500 of 1500 at 0"]);
    assert!(app.errors.borrow().is_empty());
    fs::remove_dir_all(&folder).expect("remove scratch folder");
}

#[test]
fn learns_the_size_once_saved_when_the_server_never_says() {
    let folder = scratch_folder("download-no-length");
    let server = Server::start(Resource::new("twelve bytes").without_length(), []);
    let app = start(&detailed_downloader(&server.url("/a.txt"), &folder));

    app.click("Get");
    app.app.wait_for_downloads();
    app.fake.run_pending_wakes();

    assert_eq!(app.labels(), ["100% of 12 bytes", "12 of 12 at 0"]);
    assert_eq!(
        fs::read_to_string(folder.join("a.txt")).expect("read the saved file"),
        "twelve bytes"
    );
    fs::remove_dir_all(&folder).expect("remove scratch folder");
}

const BODY: &str = "0123456789";

/// Keeps each download in a list, by id, with buttons to cancel and remove the first one.
fn list_downloader(url: &str, folder: &Path) -> String {
    list_downloader_with(url, folder, "")
}

/// [`list_downloader`] with `settings` added to the call.
fn list_downloader_with(url: &str, folder: &Path, settings: &str) -> String {
    format!(
        "struct Item {{\n  id: string\n  percent: int\n  status: string\n  received: int\n}}\n\
         state items: [Item] = []\n\
         fn get() {{\n\
           items.push(Item(id: \"\", percent: 0, status: \"\", received: 0))\n\
           let last = items.len() - 1\n\
           download(\"{url}\", to: {}, progress: $items[last].percent, status: $items[last].status, received: $items[last].received, id: $items[last].id{settings})\n\
         }}\n\
         window(\"Downloader\") {{\n\
           button(\"Get\") {{ get() }}\n\
           button(\"Cancel first\") {{ cancel_download(items[0].id) }}\n\
           button(\"Pause first\") {{ pause_download(items[0].id) }}\n\
           button(\"Resume first\") {{ resume_download(items[0].id) }}\n\
           button(\"Remove first\") {{ items.remove(0) }}\n\
           for item in items {{\n\
             label(\"{{item.id}}: {{item.received}} {{item.status}}\")\n\
           }}\n\
         }}",
        nifty_syntax::string_literal(&folder.display().to_string())
    )
}

/// Every file in `folder`, by name.
fn files_in(folder: &Path) -> Vec<String> {
    let mut names: Vec<_> = fs::read_dir(folder)
        .map(|entries| {
            entries
                .map(|entry| {
                    entry
                        .expect("read a folder entry")
                        .file_name()
                        .to_string_lossy()
                        .into_owned()
                })
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

#[test]
fn cancelling_stops_the_download_and_removes_its_file() {
    let folder = scratch_folder("download-cancel");
    let (stall, stalled) = Step::stall(4);
    let server = Server::start(Resource::new(BODY), [stall]);
    let app = start(&list_downloader(&server.url("/a.txt"), &folder));

    app.click("Get");
    stalled.wait_until_sent();
    app.app.wait_for_download_report();
    app.fake.run_pending_wakes();
    let [shown] = &app.labels()[..] else {
        panic!("one item is shown");
    };
    let id = shown
        .split(':')
        .next()
        .expect("the label starts with the id");
    assert_eq!(*shown, format!("{id}: 4 Downloading…"));
    assert_eq!(
        files_in(&folder),
        ["a.txt.part"],
        "the partial file is there"
    );

    app.click("Cancel first");
    app.app.wait_for_downloads();
    app.fake.run_pending_wakes();

    assert_eq!(app.labels(), [format!("{id}: 4 Cancelled")]);
    assert_eq!(files_in(&folder), Vec::<String>::new());
    assert!(app.errors.borrow().is_empty());
    fs::remove_dir_all(&folder).expect("remove scratch folder");
}

#[test]
fn reports_follow_their_item_when_items_before_it_go() {
    let folder = scratch_folder("download-follows");
    let (stall, stalled) = Step::stall(4);
    let server = Server::start(Resource::new(BODY), [Step::Serve, stall]);
    let app = start(&list_downloader(&server.url("/a.txt"), &folder));

    app.click("Get");
    app.app.wait_for_downloads();
    app.click("Get");
    stalled.wait_until_sent();
    app.app.wait_for_download_report();
    app.fake.run_pending_wakes();
    let second = app.labels()[1].clone();

    app.click("Remove first");
    stalled.resume();
    app.app.wait_for_downloads();
    app.fake.run_pending_wakes();

    let id = second
        .split(':')
        .next()
        .expect("the label starts with the id");
    let saved = folder.join("a (1).txt");
    assert_eq!(
        app.labels(),
        [format!("{id}: 10 Saved to {}", saved.display())],
        "the second download's reports reach its item, now first"
    );
    assert_eq!(
        fs::read_to_string(&saved).expect("read the saved file"),
        BODY
    );
    assert!(app.errors.borrow().is_empty());
    fs::remove_dir_all(&folder).expect("remove scratch folder");
}

#[test]
fn removing_its_item_cancels_a_download() {
    let folder = scratch_folder("download-removed");
    let (stall, stalled) = Step::stall(4);
    let server = Server::start(Resource::new(BODY), [stall]);
    let app = start(&list_downloader(&server.url("/a.txt"), &folder));

    app.click("Get");
    stalled.wait_until_sent();
    app.app.wait_for_download_report();
    app.fake.run_pending_wakes();
    app.click("Remove first");
    app.app.wait_for_downloads();
    app.fake.run_pending_wakes();

    assert_eq!(app.labels(), Vec::<String>::new());
    assert_eq!(files_in(&folder), Vec::<String>::new());
    assert!(app.errors.borrow().is_empty());
    fs::remove_dir_all(&folder).expect("remove scratch folder");
}

#[test]
fn cancelling_an_id_that_is_not_running_does_nothing() {
    let folder = scratch_folder("download-cancel-finished");
    let server = Server::start(Resource::new(BODY), []);
    let app = start(&list_downloader(&server.url("/a.txt"), &folder));

    app.click("Get");
    app.app.wait_for_downloads();
    app.fake.run_pending_wakes();
    app.click("Cancel first");

    let saved = folder.join("a.txt");
    assert!(app.labels()[0].ends_with(&format!("Saved to {}", saved.display())));
    assert!(saved.exists());
    assert!(app.errors.borrow().is_empty());
    fs::remove_dir_all(&folder).expect("remove scratch folder");
}

/// Starts a download from `server` that stalls after 4 bytes, and pauses it there.
fn paused_download(server: &Server, folder: &Path, stalled: Stalled) -> (Running, String) {
    let app = start(&list_downloader(&server.url("/a.txt"), folder));
    app.click("Get");
    stalled.wait_until_sent();
    app.app.wait_for_download_report();
    app.fake.run_pending_wakes();
    app.click("Pause first");
    app.app.wait_for_download_report();
    app.fake.run_pending_wakes();
    // The paused download has let go of this connection; closing it changes nothing.
    drop(stalled);
    let id = app.labels()[0]
        .split(':')
        .next()
        .expect("the label starts with the id")
        .to_owned();
    assert_eq!(app.labels(), [format!("{id}: 4 Paused")]);
    assert_eq!(files_in(folder), ["a.txt.part"], "the partial file stays");
    (app, id)
}

#[test]
fn resuming_asks_for_the_rest_of_the_file() {
    let folder = scratch_folder("download-resume");
    let (stall, stalled) = Step::stall(4);
    let server = Server::start(Resource::new(BODY), [stall]);
    let (app, id) = paused_download(&server, &folder, stalled);

    app.click("Resume first");
    app.app.wait_for_downloads();
    app.fake.run_pending_wakes();

    let saved = folder.join("a.txt");
    assert_eq!(
        app.labels(),
        [format!("{id}: 10 Saved to {}", saved.display())]
    );
    assert_eq!(
        fs::read_to_string(&saved).expect("read the saved file"),
        BODY
    );
    assert_eq!(files_in(&folder), ["a.txt"]);
    let resumed = &server.seen()[1];
    assert!(
        resumed.headers.contains(&"range: bytes=4-".to_owned()),
        "{resumed:?}"
    );
    assert!(app.errors.borrow().is_empty());
    fs::remove_dir_all(&folder).expect("remove scratch folder");
}

#[test]
fn resuming_starts_over_when_the_server_sends_the_whole_file() {
    let folder = scratch_folder("download-resume-no-ranges");
    let (stall, stalled) = Step::stall(4);
    let server = Server::start(Resource::new(BODY).without_ranges(), [stall]);
    let (app, id) = paused_download(&server, &folder, stalled);

    app.click("Resume first");
    app.app.wait_for_downloads();
    app.fake.run_pending_wakes();

    let saved = folder.join("a.txt");
    assert_eq!(
        app.labels(),
        [format!("{id}: 10 Saved to {}", saved.display())]
    );
    assert_eq!(
        fs::read_to_string(&saved).expect("read the saved file"),
        BODY
    );
    assert!(app.errors.borrow().is_empty());
    fs::remove_dir_all(&folder).expect("remove scratch folder");
}

#[test]
fn cancelling_a_paused_download_removes_its_partial_file() {
    let folder = scratch_folder("download-cancel-paused");
    let (stall, stalled) = Step::stall(4);
    let server = Server::start(Resource::new(BODY), [stall]);
    let (app, id) = paused_download(&server, &folder, stalled);

    app.click("Cancel first");
    app.app.wait_for_downloads();
    app.fake.run_pending_wakes();

    assert_eq!(app.labels(), [format!("{id}: 4 Cancelled")]);
    assert_eq!(files_in(&folder), Vec::<String>::new());
    fs::remove_dir_all(&folder).expect("remove scratch folder");
}

/// The `Range` each request asked for, in the order they arrived.
fn ranges(server: &Server) -> Vec<Option<String>> {
    server
        .seen()
        .iter()
        .map(|seen| {
            seen.headers
                .iter()
                .find_map(|header| header.strip_prefix("range: "))
                .map(str::to_owned)
        })
        .collect()
}

#[test]
fn splits_the_file_across_connections() {
    let folder = scratch_folder("download-parts");
    let total = 4 * MIN_PART_SIZE + 3;
    let body = numbered_body(total);
    let server = Server::start(Resource::new(body.clone()), []);
    let app = start(&downloader_with(
        &server.url("/a.bin"),
        &folder,
        ", connections: 4",
    ));

    app.click("Get");
    app.app.wait_for_downloads();
    app.fake.run_pending_wakes();

    let saved = folder.join("a.bin");
    assert_eq!(app.labels(), [format!("100% Saved to {}", saved.display())]);
    assert_eq!(fs::read(&saved).expect("read the saved file"), body);
    let mut asked = ranges(&server);
    assert_eq!(
        asked.remove(0).as_deref(),
        Some("bytes=0-"),
        "the first asks for all of it as a range"
    );
    asked.sort();
    let part = |index: u64| total * index / 4;
    let mut expected: Vec<_> = (1..4)
        .map(|index| Some(format!("bytes={}-{}", part(index), part(index + 1) - 1)))
        .collect();
    expected.sort();
    assert_eq!(asked, expected);
    assert_eq!(files_in(&folder), ["a.bin"]);
    assert!(app.errors.borrow().is_empty());
    fs::remove_dir_all(&folder).expect("remove scratch folder");
}

#[test]
fn uses_one_connection_when_the_server_cannot_send_parts() {
    let folder = scratch_folder("download-parts-unsupported");
    let body = numbered_body(4 * MIN_PART_SIZE);
    let server = Server::start(Resource::new(body.clone()).without_ranges(), []);
    let app = start(&downloader_with(
        &server.url("/a.bin"),
        &folder,
        ", connections: 4",
    ));

    app.click("Get");
    app.app.wait_for_downloads();
    app.fake.run_pending_wakes();

    assert_eq!(
        fs::read(folder.join("a.bin")).expect("read the saved file"),
        body
    );
    assert_eq!(server.seen().len(), 1, "no connection after the first");
    assert!(app.errors.borrow().is_empty());
    fs::remove_dir_all(&folder).expect("remove scratch folder");
}

/// Starts a download from `server` over two connections, each stalling partway through
/// its part, and waits for its first report.
fn stalled_in_parts(server_body: &[u8], folder: &Path) -> (Server, Running, [Stalled; 2]) {
    let (first_stall, first) = Step::stall(1_000);
    let (second_stall, second) = Step::stall(1_000);
    let server = Server::start(Resource::new(server_body), [first_stall, second_stall]);
    let app = start(&list_downloader_with(
        &server.url("/a.bin"),
        folder,
        ", connections: 2",
    ));
    app.click("Get");
    first.wait_until_sent();
    second.wait_until_sent();
    app.app.wait_for_download_report();
    app.fake.run_pending_wakes();
    (server, app, [first, second])
}

#[test]
fn pausing_keeps_each_part_and_resuming_continues_them() {
    let folder = scratch_folder("download-parts-resume");
    let total = 2 * MIN_PART_SIZE;
    let body = numbered_body(total);
    let (server, app, stalls) = stalled_in_parts(&body, &folder);

    app.click("Pause first");
    app.app.wait_for_download_report();
    app.fake.run_pending_wakes();
    drop(stalls);
    assert!(app.labels()[0].ends_with("Paused"), "{:?}", app.labels());
    assert_eq!(files_in(&folder), ["a.bin.part"]);

    app.click("Resume first");
    app.app.wait_for_downloads();
    app.fake.run_pending_wakes();

    let saved = folder.join("a.bin");
    assert!(
        app.labels()[0].ends_with(&format!("{total} Saved to {}", saved.display())),
        "{:?}",
        app.labels()
    );
    assert_eq!(fs::read(&saved).expect("read the saved file"), body);
    let resumed = &ranges(&server)[2..];
    assert_eq!(resumed.len(), 2, "one connection for each part");
    for (start, end) in [(0, MIN_PART_SIZE), (MIN_PART_SIZE, total)] {
        let continues = resumed.iter().flatten().any(|range| {
            let (from, last) = range
                .strip_prefix("bytes=")
                .and_then(|span| span.split_once('-'))
                .expect("a range of bytes");
            let from: u64 = from.parse().expect("a byte number");
            from >= start && from < end && last == (end - 1).to_string()
        });
        assert!(continues, "the part from {start} continues: {resumed:?}");
    }
    assert!(app.errors.borrow().is_empty());
    fs::remove_dir_all(&folder).expect("remove scratch folder");
}

#[test]
fn cancelling_stops_every_connection_and_removes_the_file() {
    let folder = scratch_folder("download-parts-cancel");
    let body = numbered_body(2 * MIN_PART_SIZE);
    let (_server, app, _stalls) = stalled_in_parts(&body, &folder);

    app.click("Cancel first");
    app.app.wait_for_downloads();
    app.fake.run_pending_wakes();

    assert!(app.labels()[0].ends_with("Cancelled"), "{:?}", app.labels());
    assert_eq!(files_in(&folder), Vec::<String>::new());
    assert!(app.errors.borrow().is_empty());
    fs::remove_dir_all(&folder).expect("remove scratch folder");
}

#[test]
fn saves_under_the_name_the_call_chooses_without_replacing_files() {
    let folder = scratch_folder("download-name");
    fs::create_dir_all(&folder).expect("create the folder");
    fs::write(folder.join("notes.txt"), "mine").expect("write a file");
    let url = format!("{}/files/x.bin", serve_once("200 OK", "hello"));
    let app = start(&downloader_with(&url, &folder, ", name: \"notes.txt\""));

    app.click("Get");
    app.app.wait_for_downloads();
    app.fake.run_pending_wakes();

    let saved = folder.join("notes (1).txt");
    assert_eq!(app.labels(), [format!("100% Saved to {}", saved.display())]);
    assert_eq!(
        fs::read_to_string(&saved).expect("read the saved file"),
        "hello"
    );
    assert_eq!(
        fs::read_to_string(folder.join("notes.txt")).expect("read the existing file"),
        "mine"
    );
    assert!(app.errors.borrow().is_empty());
    fs::remove_dir_all(&folder).expect("remove scratch folder");
}

#[test]
fn a_chosen_name_never_leaves_the_folder() {
    let folder = scratch_folder("download-name-unsafe");
    let url = format!("{}/x.bin", serve_once("200 OK", "hello"));
    let app = start(&downloader_with(&url, &folder, ", name: \"../escape.txt\""));

    app.click("Get");
    app.app.wait_for_downloads();
    app.fake.run_pending_wakes();

    assert_eq!(files_in(&folder), [".._escape.txt"]);
    let outside = folder.parent().expect("the scratch folder has a parent");
    assert!(!outside.join("escape.txt").exists());
    assert!(app.errors.borrow().is_empty());
    fs::remove_dir_all(&folder).expect("remove scratch folder");
}

#[test]
fn sends_the_calls_headers_with_every_request() {
    let folder = scratch_folder("download-headers");
    let body = numbered_body(2 * MIN_PART_SIZE);
    let server = Server::start(Resource::new(body.clone()), []);
    let app = start(&downloader_with(
        &server.url("/a.bin"),
        &folder,
        ", connections: 2, headers: [\"Authorization: Bearer abc\", \"X-Client: nifty\"]",
    ));

    app.click("Get");
    app.app.wait_for_downloads();
    app.fake.run_pending_wakes();

    assert_eq!(
        fs::read(folder.join("a.bin")).expect("read the saved file"),
        body
    );
    let seen = server.seen();
    assert_eq!(seen.len(), 2);
    for request in &seen {
        for header in ["authorization: Bearer abc", "x-client: nifty"] {
            assert!(request.headers.contains(&header.to_owned()), "{request:?}");
        }
    }
    assert!(app.errors.borrow().is_empty());
    fs::remove_dir_all(&folder).expect("remove scratch folder");
}

#[test]
fn a_malformed_header_fails_without_asking_the_server() {
    let folder = scratch_folder("download-bad-header");
    let server = Server::start(Resource::new(BODY), []);
    let app = start(&downloader_with(
        &server.url("/a.txt"),
        &folder,
        ", headers: [\"no colon\"]",
    ));

    app.click("Get");
    app.app.wait_for_downloads();
    app.fake.run_pending_wakes();

    assert_eq!(
        app.labels(),
        [
            "0% Failed: the header `no colon` isn't written as `Name: value`, such as `Authorization: Bearer …`"
        ]
    );
    assert_eq!(server.seen(), []);
    assert!(app.errors.borrow().is_empty());
}

#[test]
fn retries_a_server_error_after_waiting() {
    let folder = scratch_folder("download-retry");
    let server = Server::start(
        Resource::new(BODY),
        [Step::Reply("503 Service Unavailable"), Step::Serve],
    );
    let app = start(&downloader_with(
        &server.url("/a.txt"),
        &folder,
        ", retries: 1",
    ));

    app.click("Get");
    app.app.wait_for_download_report();
    app.fake.run_pending_wakes();
    assert_eq!(app.labels(), ["0% Retrying…"]);
    app.app.wait_for_downloads();
    app.fake.run_pending_wakes();

    let saved = folder.join("a.txt");
    assert_eq!(app.labels(), [format!("100% Saved to {}", saved.display())]);
    assert_eq!(
        fs::read_to_string(&saved).expect("read the saved file"),
        BODY
    );
    assert_eq!(server.seen().len(), 2);
    assert!(app.errors.borrow().is_empty());
    fs::remove_dir_all(&folder).expect("remove scratch folder");
}

#[test]
fn fails_once_its_retries_run_out() {
    let folder = scratch_folder("download-retries-run-out");
    let server = Server::start(
        Resource::new(BODY),
        [
            Step::Reply("503 Service Unavailable"),
            Step::Reply("503 Service Unavailable"),
        ],
    );
    let app = start(&downloader_with(
        &server.url("/a.txt"),
        &folder,
        ", retries: 1",
    ));

    app.click("Get");
    app.app.wait_for_downloads();
    app.fake.run_pending_wakes();

    assert_eq!(app.labels(), ["0% Failed: http status: 503"]);
    assert_eq!(server.seen().len(), 2);
    assert_eq!(files_in(&folder), Vec::<String>::new());
    assert!(app.errors.borrow().is_empty());
}

#[test]
fn does_not_retry_a_failure_that_will_not_pass() {
    let folder = scratch_folder("download-no-retry");
    let server = Server::start(Resource::new(BODY), [Step::Reply("404 Not Found")]);
    let app = start(&downloader_with(
        &server.url("/a.txt"),
        &folder,
        ", retries: 3",
    ));

    app.click("Get");
    app.app.wait_for_downloads();
    app.fake.run_pending_wakes();

    assert_eq!(app.labels(), ["0% Failed: http status: 404"]);
    assert_eq!(server.seen().len(), 1);
    assert!(app.errors.borrow().is_empty());
}

/// Starts a download whose first request gets a server error, and waits until it is
/// waiting to retry.
fn waiting_to_retry(folder: &Path) -> (Server, Running) {
    let server = Server::start(
        Resource::new(BODY),
        [Step::Reply("503 Service Unavailable")],
    );
    let app = start(&list_downloader_with(
        &server.url("/a.txt"),
        folder,
        ", retries: 3",
    ));
    app.click("Get");
    app.app.wait_for_download_report();
    app.fake.run_pending_wakes();
    assert!(
        app.labels()[0].ends_with("0 Retrying…"),
        "{:?}",
        app.labels()
    );
    (server, app)
}

#[test]
fn cancelling_while_waiting_to_retry_stops_at_once() {
    let folder = scratch_folder("download-retry-cancel");
    let (server, app) = waiting_to_retry(&folder);

    app.click("Cancel first");
    app.app.wait_for_downloads();
    app.fake.run_pending_wakes();

    assert!(
        app.labels()[0].ends_with("0 Cancelled"),
        "{:?}",
        app.labels()
    );
    assert_eq!(server.seen().len(), 1, "no retry after the cancel");
    assert!(app.errors.borrow().is_empty());
}

#[test]
fn pausing_while_waiting_to_retry_waits_for_resume_instead() {
    let folder = scratch_folder("download-retry-pause");
    let (server, app) = waiting_to_retry(&folder);

    app.click("Pause first");
    app.app.wait_for_download_report();
    app.fake.run_pending_wakes();
    assert!(app.labels()[0].ends_with("0 Paused"), "{:?}", app.labels());
    app.click("Resume first");
    app.app.wait_for_downloads();
    app.fake.run_pending_wakes();

    let saved = folder.join("a.txt");
    assert!(
        app.labels()[0].ends_with(&format!("10 Saved to {}", saved.display())),
        "{:?}",
        app.labels()
    );
    assert_eq!(server.seen().len(), 2);
    assert!(app.errors.borrow().is_empty());
    fs::remove_dir_all(&folder).expect("remove scratch folder");
}
