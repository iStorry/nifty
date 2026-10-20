use std::fs;
use std::path::Path;

use nifty_download::testing::{Resource, Server, scratch_folder, serve_once};
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
