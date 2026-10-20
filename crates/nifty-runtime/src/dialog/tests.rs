use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use super::*;
use crate::testing::FakeBackend;
use crate::ui::Ui;

/// Dialogs on a fake backend whose waker counts how often it ran.
fn dialogs(fake: &FakeBackend) -> (Dialogs<&'static str>, Rc<RefCell<usize>>) {
    let ui = Ui::new(fake.clone());
    let wake_count = Rc::new(RefCell::new(0));
    let waker = ui.waker({
        let wake_count = Rc::clone(&wake_count);
        Box::new(move || *wake_count.borrow_mut() += 1)
    });
    (Dialogs::new(ui.dialogs(), Arc::new(waker)), wake_count)
}

fn open_file() -> FileDialog {
    FileDialog::new(DialogKind::File, String::new(), &[], "", String::new())
        .expect("no folder needs no home folder")
}

fn received(dialogs: &Dialogs<&'static str>) -> Vec<(&'static str, Picked)> {
    let mut answers = Vec::new();
    dialogs
        .receive(|target, picked| {
            answers.push((*target, picked));
            Ok::<(), ()>(())
        })
        .expect("recording answers never fails");
    answers
}

#[test]
fn a_dialog_shows_only_once_the_event_has_finished() {
    let fake = FakeBackend::default();
    let (dialogs, wakes) = dialogs(&fake);

    dialogs.ask(open_file(), "photo");
    assert!(fake.dialogs().is_empty(), "asking shows nothing yet");
    fake.run_pending_wakes();
    assert_eq!(*wakes.borrow(), 1, "asking wakes the UI thread");

    dialogs.show_asked();
    assert_eq!(fake.dialogs(), [open_file()]);
    dialogs.show_asked();
    assert_eq!(fake.dialogs().len(), 1, "each dialog shows once");
}

#[test]
fn answers_come_back_through_the_waker_in_order() {
    let fake = FakeBackend::default();
    let (dialogs, wakes) = dialogs(&fake);
    dialogs.ask(open_file(), "first");
    dialogs.ask(open_file(), "second");
    dialogs.show_asked();
    fake.run_pending_wakes();

    fake.close_dialog(1, Picked::Cancelled);
    fake.close_dialog(0, Picked::Chosen(vec!["/a.txt".to_owned()]));
    fake.run_pending_wakes();

    assert_eq!(*wakes.borrow(), 2);
    assert_eq!(
        received(&dialogs),
        [
            ("second", Picked::Cancelled),
            ("first", Picked::Chosen(vec!["/a.txt".to_owned()])),
        ]
    );
    assert_eq!(received(&dialogs), []);
}

#[test]
fn receiving_stops_at_the_first_error_and_keeps_the_rest() {
    let fake = FakeBackend::default();
    let (dialogs, _) = dialogs(&fake);
    dialogs.ask(open_file(), "first");
    dialogs.ask(open_file(), "second");
    dialogs.show_asked();
    fake.close_dialog(0, Picked::Cancelled);
    fake.close_dialog(1, Picked::Cancelled);

    let failed = dialogs.receive(|target, _| Err(*target));
    assert_eq!(failed, Err("first"));
    assert_eq!(received(&dialogs), [("second", Picked::Cancelled)]);
}

#[test]
fn file_types_are_kept_without_their_dot() {
    let dialog = FileDialog::new(
        DialogKind::SavePath,
        "Export".to_owned(),
        &[".png", "jpg", "", "."],
        "",
        "photo.png".to_owned(),
    )
    .expect("no folder needs no home folder");
    assert_eq!(dialog.types, ["png", "jpg"]);
    assert_eq!(dialog.folder, None);
}

#[test]
fn only_an_existing_folder_is_where_the_dialog_opens() {
    let open_in = |folder: &str| {
        FileDialog::new(
            DialogKind::Folder,
            String::new(),
            &[],
            folder,
            String::new(),
        )
        .expect("tests run with a home folder")
        .folder
    };
    let home = std::env::home_dir().expect("tests run with a home folder");
    assert_eq!(open_in("~"), Some(home));
    let temp = std::env::temp_dir();
    assert_eq!(open_in(&temp.display().to_string()), Some(temp.clone()));
    let missing = temp.join("nf-test-no-such-folder");
    assert_eq!(open_in(&missing.display().to_string()), None);
}
