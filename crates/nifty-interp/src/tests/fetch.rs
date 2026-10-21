use nifty_download::testing::{Resource, Server, Step, serve_once};
use nifty_runtime::testing::FakeKind;

use super::{Running, start};

/// An app whose button fetches `url` from a function, so its block sees the function's
/// parameter, and shows what came back.
fn fetcher(url: &str, settings: &str) -> String {
    format!(
        "state status = \"\"\nstate page = \"\"\nstate code = 0\nstate headers: [string] = []\nstate found = \"\"\n\
         fn get(name: string) {{\n\
           fetch(\"{url}\", status: $status, text: $page, code: $code, response_headers: $headers{settings}) {{\n\
             found = \"{{name}} saw {{code}}\"\n\
           }}\n\
         }}\n\
         window(\"Fetcher\") {{\n\
           button(\"Get\") {{ get(\"first\") }}\n\
           label(status)\n\
           label(page)\n\
           label(found)\n\
         }}"
    )
}

fn fetched(app: &Running) {
    app.app.wait_for_fetches();
    app.fake.run_pending_wakes();
}

#[test]
fn reads_the_page_into_state_then_runs_the_block_with_the_names_it_saw() {
    let server = Server::start(
        Resource::new("hello, nifty").with_header("Set-Cookie: token=abc; Path=/"),
        [],
    );
    let url = server.url("/page");
    let app = start(&fetcher(&url, ", headers: [\"X-Test: yes\"]"));

    app.click("Get");
    assert_eq!(
        app.labels(),
        ["Fetching…", "", ""],
        "the click shows it has begun"
    );

    fetched(&app);
    assert_eq!(app.labels(), ["Fetched", "hello, nifty", "first saw 200"]);
    let headers = app.state(3);
    assert!(
        headers.as_strs().contains(&"set-cookie: token=abc; Path=/"),
        "{headers:?}"
    );
    let seen = server.seen();
    assert!(
        seen[0].headers.contains(&"x-test: yes".to_owned()),
        "{seen:?}"
    );
    assert!(app.errors.borrow().is_empty());
}

#[test]
fn an_error_status_fails_but_keeps_the_body_and_still_runs_the_block() {
    let url = serve_once("404 Not Found", "no such video");
    let app = start(&fetcher(&url, ""));

    app.click("Get");
    fetched(&app);

    assert_eq!(
        app.labels(),
        ["Failed: http status: 404", "no such video", "first saw 404"]
    );
}

#[test]
fn no_response_fails_with_the_reason() {
    let app = start(&fetcher("http://127.0.0.1:1/closed", ""));

    app.click("Get");
    fetched(&app);

    let labels = app.labels();
    assert!(labels[0].starts_with("Failed: "), "{labels:?}");
    assert_eq!(labels[1..], ["", "first saw 0"]);
}

#[test]
fn a_header_that_cant_be_sent_fails_before_asking() {
    let server = Server::start(Resource::new("unseen"), [Step::Serve]);
    let app = start(&fetcher(&server.url("/"), ", headers: [\"no colon\"]"));

    app.click("Get");
    fetched(&app);

    assert_eq!(
        app.labels()[0],
        "Failed: the header `no colon` isn't written as `Name: value`, such as `Authorization: Bearer …`"
    );
    assert_eq!(server.seen(), Vec::new());
}

#[test]
fn the_block_keeps_the_item_a_loop_gave_it_when_the_list_changes() {
    let url = serve_once("200 OK", "ok");
    let app = start(&format!(
        "struct Video {{\n  name: string\n}}\n\
         state videos = [Video(name: \"cat\"), Video(name: \"dog\")]\n\
         state status = \"\"\nstate found = \"\"\n\
         window(\"Fetcher\") {{\n\
           for video in videos {{\n\
             button(\"Get {{video.name}}\") {{\n\
               fetch(\"{url}\", status: $status) {{ found = video.name }}\n\
               videos.remove(0)\n\
             }}\n\
           }}\n\
           label(found)\n\
         }}"
    ));

    app.click("Get dog");
    fetched(&app);

    assert_eq!(app.labels(), ["dog"]);
    assert!(app.errors.borrow().is_empty());
}

#[test]
fn strings_are_searched_and_cut_by_character() {
    let app = start(
        r#"state page = "<a>héllo, wörld</a>"
state padded = "  padded "
state json = "[1, [\"a\\u002Fb\"]]"
fn facts() -> string {
  let pieces = page.split(",").len()
  let has = page.contains("llo")
  let starts = page.starts_with("<a")
  let ends = page.ends_with("b>")
  return "{pieces} {has} {starts} {ends}"
}
window("Text") {
  label(page.slice(page.find(">") + 1, page.find("</")).replace("wörld", "nifty"))
  label(facts())
  label(padded.trim())
  label(json_text(json, ["1", "0"]))
}"#,
    );
    assert_eq!(
        app.labels(),
        ["héllo, nifty", "2 true true false", "padded", "a/b"]
    );
}

#[test]
fn a_slice_outside_the_text_is_a_runtime_error() {
    let app = start(
        "state page = \"abc\"\nstate cut = \"\"\n\
         window(\"Text\") {\n\
           button(\"Cut\") { cut = page.slice(2, 9) }\n\
         }",
    );
    app.click("Cut");
    assert_eq!(
        *app.errors.borrow(),
        [
            "can't slice from 2 to 9; the text has 3 characters, and a slice needs 0 <= start <= end <= len"
        ]
    );
}

#[test]
fn the_preview_example_shows_a_pages_title_and_type() {
    let server = Server::start(
        Resource::new("<html><head><title> Hello, nifty </title></head></html>")
            .with_header("Content-Type: text/html"),
        [],
    );
    let app = start(include_str!("../../../../fixtures/preview.nf"));
    let field = app.only(FakeKind::TextField);
    app.fake.type_text(field, &server.url("/"));
    app.click("Preview");
    fetched(&app);

    assert_eq!(
        app.labels(),
        [
            "Fetched",
            "Hello, nifty",
            "text/html · 55 characters · HTTP 200"
        ]
    );
    assert!(app.errors.borrow().is_empty());
}
