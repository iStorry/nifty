use std::fs;
use std::path::Path;

use nifty_download::testing::{Resource, Server, Step, scratch_folder, serve_once};
use nifty_runtime::testing::FakeKind;

use super::start;

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
    format!(
        "struct Item {{\n  id: string\n  percent: int\n  status: string\n  received: int\n}}\n\
         state items: [Item] = []\n\
         fn get() {{\n\
           items.push(Item(id: \"\", percent: 0, status: \"\", received: 0))\n\
           let last = items.len() - 1\n\
           download(\"{url}\", to: {}, progress: $items[last].percent, status: $items[last].status, received: $items[last].received, id: $items[last].id)\n\
         }}\n\
         window(\"Downloader\") {{\n\
           button(\"Get\") {{ get() }}\n\
           button(\"Cancel first\") {{ cancel_download(items[0].id) }}\n\
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
    assert_eq!(files_in(&folder), ["a.txt"], "the partial file is there");

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
