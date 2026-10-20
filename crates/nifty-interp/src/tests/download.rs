use std::fs;
use std::path::Path;

use nifty_download::testing::{scratch_folder, serve_once};
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
