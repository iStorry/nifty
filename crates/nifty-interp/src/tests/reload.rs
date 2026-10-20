use std::cell::RefCell;
use std::fs;
use std::rc::Rc;

use nifty_download::testing::{Resource, Server, Step, scratch_folder};

use nifty_runtime::testing::{FakeBackend, FakeKind};
use nifty_runtime::{Size, Waker as _};

use super::{Running, program, record_errors, start};
use crate::{LiveApp, RuntimeError, Value};

const COUNTER: &str = include_str!("../../../../fixtures/counter.nf");
const TODOS: &str = include_str!("../../../../fixtures/todos.nf");

impl Running {
    /// Replaces the running app with `source`, as `nf dev` does after a save.
    fn reload(&mut self, source: &str) -> Result<(), RuntimeError> {
        self.app = self.app.reload(program(source))?;
        Ok(())
    }
}

#[test]
fn state_with_the_same_name_and_type_survives() {
    let mut app = start(COUNTER);
    app.click("+");
    app.click("+");

    app.reload(&COUNTER.replace("Clicked {count} times", "Count: {count}"))
        .expect("the new version starts");

    assert_eq!(app.labels(), ["Count: 2"]);
    app.click("+");
    assert_eq!(app.labels(), ["Count: 3"], "the new version handles events");
    assert!(app.errors.borrow().is_empty());
}

#[test]
fn state_whose_type_changed_starts_over() {
    let mut app = start(COUNTER);
    app.click("+");

    app.reload(
        &COUNTER
            .replace("state count = 0", "state count = 0.5")
            .replace("count -= 1", "count -= 1.0")
            .replace("count += 1", "count += 1.0"),
    )
    .expect("the new version starts");

    assert_eq!(app.state(0), Value::Float(0.5));
}

#[test]
fn struct_state_survives_only_while_its_fields_stay_the_same() {
    let mut app = start(TODOS);
    let field = app.only(FakeKind::TextField);
    app.fake.type_text(field, "Milk");
    app.click("Add");

    app.reload(&TODOS.replace("\"Add\"", "\"Add todo\""))
        .expect("the same struct starts");
    assert_eq!(app.checkboxes(), ["Milk"]);

    app.reload(
        &TODOS
            .replace("done: bool", "done: bool\n  note: string")
            .replace(
                "Todo(title: draft, done: false)",
                "Todo(title: draft, done: false, note: \"\")",
            ),
    )
    .expect("the changed struct starts");
    assert!(app.checkboxes().is_empty(), "a changed struct starts over");
}

#[test]
fn a_failed_reload_leaves_the_app_running() {
    let mut app = start(COUNTER);
    app.click("+");
    let shown = app.fake.find(FakeKind::Label);

    let error = app
        .reload(&COUNTER.replace(
            "label(\"Clicked {count} times\"",
            "label(\"{10 / (count - 1)}\"",
        ))
        .expect_err("building the new UI divides by zero");

    assert_eq!(error.message, "can't divide by zero");
    assert_eq!(app.fake.find(FakeKind::Label), shown);
    app.click("+");
    assert_eq!(app.labels(), ["Clicked 2 times"]);
}

#[test]
fn a_failed_reload_closes_the_windows_it_opened() {
    let mut app = start(COUNTER);

    app.reload(&format!(
        "{COUNTER}\nwindow(\"Extra\") {{ label(\"{{1 / (count - count)}}\") }}"
    ))
    .expect_err("the extra window divides by zero");

    assert_eq!(app.fake.window_count(), 2);
    assert!(app.fake.is_closed(1));
    assert!(!app.fake.is_closed(0));
}

#[test]
fn windows_are_reused_retitled_added_and_closed() {
    let mut app = start(COUNTER);

    app.reload(&format!(
        "{}\nwindow(\"Extra\") {{ label(\"extra\") }}",
        COUNTER.replace("\"Counter\"", "\"Tally\"")
    ))
    .expect("two windows start");
    assert_eq!(app.fake.window_count(), 2);
    assert_eq!(app.fake.window_title(0), "Tally");
    assert!(app.fake.is_shown(1));

    app.reload(COUNTER).expect("one window starts");
    assert!(app.fake.is_closed(1));
    assert!(!app.fake.is_closed(0));
    assert_eq!(app.labels(), ["Clicked 0 times"]);
}

#[test]
fn only_a_changed_declared_size_resizes_the_window() {
    let mut app = start(COUNTER);
    let resized_by_user = Size::new(500.0, 400.0);
    app.fake.resize(0, resized_by_user);

    app.reload(&COUNTER.replace("Clicked", "Pressed"))
        .expect("the new version starts");
    assert_eq!(app.fake.window_size(0), resized_by_user);

    app.reload(&COUNTER.replace("width: 320", "width: 360"))
        .expect("the new version starts");
    assert_eq!(app.fake.window_size(0), Size::new(360.0, 200.0));
}

#[test]
fn a_live_app_loads_new_versions_when_woken() {
    let fake = FakeBackend::default();
    let errors = Rc::new(RefCell::new(Vec::new()));
    let live = Rc::new(LiveApp::new(fake.clone(), record_errors(&errors)));
    let source = Rc::new(RefCell::new(COUNTER.to_owned()));
    let waker = live.waker(Box::new({
        let live = Rc::downgrade(&live);
        let source = Rc::clone(&source);
        move || {
            if let Some(live) = live.upgrade() {
                live.load(program(&source.borrow()))
                    .expect("the new version starts");
            }
        }
    }));

    live.load(program(COUNTER))
        .expect("the first version starts");
    assert_eq!(fake.visible_texts(FakeKind::Label), ["Clicked 0 times"]);

    *source.borrow_mut() = COUNTER.replace("Clicked", "Pressed");
    fake.run_pending_wakes();
    assert_eq!(
        fake.visible_texts(FakeKind::Label),
        ["Clicked 0 times"],
        "nothing reloads until woken"
    );

    waker.wake();
    fake.run_pending_wakes();
    assert_eq!(fake.visible_texts(FakeKind::Label), ["Pressed 0 times"]);
    assert!(errors.borrow().is_empty());
}

#[test]
fn reloading_stops_running_downloads_and_removes_their_files() {
    let folder = scratch_folder("reload-download");
    let (stall, stalled) = Step::stall(4);
    let server = Server::start(Resource::new("0123456789"), [stall]);
    let source = format!(
        "state status = \"\"\nstate percent = 0\nwindow(\"Downloader\") {{\n  button(\"Get\") {{ download(\"{}\", to: {}, progress: $percent, status: $status) }}\n  label(status)\n}}",
        server.url("/a.txt"),
        nifty_syntax::string_literal(&folder.display().to_string())
    );
    let mut app = start(&source);
    app.click("Get");
    stalled.wait_until_sent();
    app.app.wait_for_download_report();
    assert!(
        folder.join("a.txt.part").exists(),
        "the partial file is there"
    );

    app.reload(&source.replace("label(status)", "label(\"Status: {status}\")"))
        .expect("the new version starts");

    assert!(
        !folder.join("a.txt.part").exists(),
        "the old program's download removed its file as it stopped"
    );
    assert_eq!(
        app.labels(),
        ["Status: Downloading…"],
        "its state keeps the last value it had"
    );
    fs::remove_dir_all(&folder).expect("remove scratch folder");
}
