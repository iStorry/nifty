use nifty_runtime::{DialogKind, FileDialog, Picked};

use super::{Running, start};

const FILES: &str = include_str!("../../../../fixtures/files.nf");

fn chosen(path: &str) -> Picked {
    Picked::Chosen(vec![path.to_owned()])
}

impl Running {
    /// Clicks `title`, then plays the UI thread once the click's event has finished.
    fn click_and_show(&self, title: &str) {
        self.click(title);
        self.fake.run_pending_wakes();
    }

    /// Closes the `index`th dialog shown with `picked`, then plays the UI thread.
    fn close_dialog(&self, index: usize, picked: Picked) {
        self.fake.close_dialog(index, picked);
        self.fake.run_pending_wakes();
    }
}

#[test]
fn shows_the_dialog_only_once_the_click_has_finished() {
    let app = start(FILES);

    app.click("Choose photo…");
    assert_eq!(app.fake.dialogs(), [], "the click only asks for it");
    app.fake.run_pending_wakes();

    let expected = FileDialog::new(
        DialogKind::File,
        "Choose a photo".to_owned(),
        &["png", "jpg"],
        "",
        String::new(),
    )
    .expect("no folder needs no home folder");
    assert_eq!(app.fake.dialogs(), [expected]);
}

#[test]
fn writes_the_chosen_path_into_the_bound_state() {
    let app = start(FILES);
    app.click_and_show("Choose photo…");

    app.close_dialog(0, chosen("/photos/cat.png"));

    assert!(app.labels().contains(&"/photos/cat.png".to_owned()));
    assert!(app.errors.borrow().is_empty());
}

#[test]
fn cancelling_leaves_the_state_as_it_was() {
    let app = start(FILES);
    app.click_and_show("Choose photo…");
    app.close_dialog(0, chosen("/photos/cat.png"));

    app.click_and_show("Choose photo…");
    app.close_dialog(1, Picked::Cancelled);

    assert!(app.labels().contains(&"/photos/cat.png".to_owned()));
    assert!(app.errors.borrow().is_empty());
}

#[test]
fn picking_files_replaces_the_bound_list() {
    let app = start(FILES);
    app.click_and_show("Attach files…");
    assert_eq!(app.fake.dialogs()[0].kind, DialogKind::Files);

    app.close_dialog(
        0,
        Picked::Chosen(vec!["/a.txt".to_owned(), "/b.txt".to_owned()]),
    );

    assert!(app.labels().contains(&"2 attached".to_owned()));
}

#[test]
fn folder_and_save_dialogs_pass_their_settings_on() {
    let app = start(FILES);
    app.click_and_show("Choose folder…");
    app.click_and_show("Export…");

    let [folder, save] = &app.fake.dialogs()[..] else {
        panic!("both clicks show a dialog");
    };
    assert_eq!(folder.kind, DialogKind::Folder);
    assert_eq!(folder.folder, std::env::home_dir());
    assert_eq!(save.kind, DialogKind::SavePath);
    assert_eq!(save.name, "export.csv");
    assert_eq!(save.types, ["csv"]);
}

#[test]
fn a_dialog_that_fails_reports_an_error() {
    let app = start(FILES);
    app.click_and_show("Choose photo…");

    app.close_dialog(0, Picked::Failed("no display".to_owned()));

    assert_eq!(*app.errors.borrow(), ["the file dialog failed: no display"]);
}

#[test]
fn a_dialog_cannot_answer_into_a_variable() {
    let app = start(
        "fn choose(paths: [string]) {\n  for path in paths {\n    pick_file($path)\n  }\n}\n\
         window(\"A\") {\n  button(\"Open\") { choose([\"\"]) }\n}",
    );
    app.click_and_show("Open");

    assert_eq!(app.fake.dialogs(), []);
    assert_eq!(
        *app.errors.borrow(),
        ["a file dialog can only answer into state, not into a variable"]
    );
}
