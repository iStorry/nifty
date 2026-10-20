use std::cell::RefCell;
use std::rc::Rc;

use nifty_check::hir::{Program, StateId};
use nifty_runtime::testing::{FakeBackend, FakeKind};
use nifty_runtime::{Backdrop, Ui};
use nifty_syntax::FileId;

use crate::{App, ErrorHandler, RuntimeError, Value};

struct Running {
    app: App<FakeBackend>,
    fake: FakeBackend,
    errors: Rc<RefCell<Vec<String>>>,
}

mod dialog;
mod download;
mod reload;

fn program(source: &str) -> Program {
    let parsed = nifty_syntax::parse(source, FileId::default());
    assert_eq!(parsed.diagnostics, [], "the test source must parse");
    let checked = nifty_check::check(&[&parsed.file]);
    assert_eq!(checked.diagnostics, [], "the test source must check");
    checked.program
}

/// An error handler that records each message in `errors`.
fn record_errors(errors: &Rc<RefCell<Vec<String>>>) -> ErrorHandler {
    let errors = Rc::clone(errors);
    Rc::new(move |error: RuntimeError| errors.borrow_mut().push(error.message))
}

fn start(source: &str) -> Running {
    let fake = FakeBackend::default();
    let errors = Rc::new(RefCell::new(Vec::new()));
    let app = App::start(
        program(source),
        Ui::new(fake.clone()),
        record_errors(&errors),
    )
    .expect("the test app starts");
    Running { app, fake, errors }
}

impl Running {
    fn labels(&self) -> Vec<String> {
        self.fake.visible_texts(FakeKind::Label)
    }

    fn checkboxes(&self) -> Vec<String> {
        self.fake.visible_texts(FakeKind::Checkbox)
    }

    /// Clicks the live button with this title.
    fn click(&self, title: &str) {
        let button = self
            .fake
            .find(FakeKind::Button)
            .into_iter()
            .find(|button| self.fake.text(*button) == title)
            .unwrap_or_else(|| panic!("no button titled {title:?}"));
        self.fake.click(button);
    }

    fn only(&self, kind: FakeKind) -> usize {
        let found = self.fake.find(kind);
        assert_eq!(found.len(), 1, "expected exactly one {kind:?}");
        found[0]
    }

    fn state(&self, index: usize) -> Value {
        self.app.machine().state(StateId(index))
    }
}

#[test]
fn runs_the_counter_example() {
    let app = start(include_str!("../../../fixtures/counter.nf"));
    assert!(app.fake.is_shown(0));
    assert_eq!(app.fake.window_title(0), "Counter");
    assert_eq!(app.labels(), ["Clicked 0 times"]);

    app.click("+");
    app.click("+");
    assert_eq!(app.labels(), ["Clicked 2 times"]);

    app.click("−");
    app.click("−");
    app.click("−");
    assert_eq!(app.labels(), ["Clicked -1 times"]);
}

#[test]
fn a_rebuilt_surface_stays_behind_its_children() {
    let app = start(
        r#"
state framed = true
window("Test") {
  if framed {
    column(surface: .card) { label("inside") }
  }
  button("Flip") { framed = !framed }
}
"#,
    );
    let first = app.only(FakeKind::Surface);
    assert_eq!(app.fake.style(first).as_deref(), Some("Card"));
    assert!(
        first < app.only(FakeKind::Label),
        "created before its label"
    );

    app.click("Flip");
    assert_eq!(app.fake.find(FakeKind::Surface), Vec::<usize>::new());
    assert!(app.fake.is_removed(first));

    app.click("Flip");
    let second = app.only(FakeKind::Surface);
    assert!(
        second < app.only(FakeKind::Label),
        "created before its label"
    );
}

#[test]
fn nav_items_follow_and_set_their_selection() {
    let app = start(include_str!("../../../fixtures/sidebar.nf"));
    let items = app.fake.find(FakeKind::NavItem);
    let picked = |app: &Running| -> Vec<bool> {
        items
            .iter()
            .map(|&item| app.fake.is_checked(item))
            .collect()
    };
    assert_eq!(app.fake.text(items[0]), "Inbox");
    assert_eq!(app.fake.icon(items[0]), nifty_runtime::Icon::Inbox);
    assert_eq!(app.fake.badge(items[0]), "3");
    assert_eq!(picked(&app), [true, false, false]);

    app.fake.click(items[2]);
    assert_eq!(picked(&app), [false, false, true]);
    assert!(app.labels().contains(&"Welcome to archive".to_owned()));

    app.click("Mark all read");
    assert_eq!(app.fake.badge(items[0]), "0");
}

#[test]
fn runs_the_todos_example() {
    let app = start(include_str!("../../../fixtures/todos.nf"));
    assert_eq!(app.labels(), ["Todos", "0 left"]);

    let field = app.only(FakeKind::TextField);
    assert_eq!(app.fake.placeholder(field), "What needs doing?");
    app.fake.type_text(field, "Milk");
    app.click("Add");
    app.fake.type_text(field, "Bread");
    app.click("Add");

    assert_eq!(app.checkboxes(), ["Milk", "Bread"]);
    assert_eq!(app.labels(), ["Todos", "2 left"]);
    assert_eq!(app.fake.text(field), "", "adding clears the draft");

    let milk = app.fake.find(FakeKind::Checkbox)[0];
    app.fake.toggle(milk);
    assert_eq!(app.labels(), ["Todos", "1 left"]);
    let milk = app.fake.find(FakeKind::Checkbox)[0];
    assert!(
        app.fake.is_checked(milk),
        "the rebuilt row keeps its checked state"
    );
    assert!(app.errors.borrow().is_empty());
}

#[test]
fn runs_the_gallery_example() {
    let app = start(include_str!("../../../fixtures/gallery.nf"));
    assert_eq!(app.fake.window_title(0), "Gallery");
    let backdrops: Vec<_> = (0..app.fake.window_count())
        .map(|window| app.fake.backdrop(window))
        .collect();
    assert_eq!(
        backdrops,
        [Backdrop::Standard, Backdrop::Translucent, Backdrop::Solid]
    );
    let has_label = |text: &str| app.labels().iter().any(|label| label == text);
    assert!(has_label("Clicked 0 times"));
    assert!(has_label("Type something above"));
    assert!(has_label("You're subscribed"));

    app.click("Normal");
    app.click("Primary");
    assert!(has_label("Clicked 2 times"));
    app.click("Reset");
    assert!(has_label("Clicked 0 times"));

    let [name, notes] = app.fake.find(FakeKind::TextField)[..] else {
        panic!("the gallery has a name field and a notes field");
    };
    assert_eq!(app.fake.lines(notes), 3);
    assert_eq!(app.fake.text(notes), "Several lines\nof notes");
    app.fake.type_text(name, "Ada");
    assert!(has_label("Hello, Ada!"));

    let subscribe = app.fake.find(FakeKind::Checkbox)[0];
    app.fake.toggle(subscribe);
    assert!(has_label("You're not subscribed"));
    assert!(app.errors.borrow().is_empty());
}

#[test]
fn if_regions_appear_between_their_siblings() {
    let app = start(
        "state open = false
         window(\"Test\") {
           label(\"top\")
           if open { label(\"middle\") } else { label(\"closed\") }
           label(\"bottom\")
           button(\"Toggle\") { open = !open }
         }",
    );
    assert_eq!(app.labels(), ["top", "closed", "bottom"]);
    app.click("Toggle");
    assert_eq!(app.labels(), ["top", "middle", "bottom"]);
    app.click("Toggle");
    assert_eq!(app.labels(), ["top", "closed", "bottom"]);
}

#[test]
fn for_regions_rebuild_when_their_list_changes() {
    let app = start(
        "state names: [string] = [\"a\"]
         window(\"Test\") {
           for name in names { label(name) }
           label(\"end\")
           button(\"More\") { names.push(\"b\") }
           button(\"Fewer\") { names.remove(0) }
         }",
    );
    app.click("More");
    assert_eq!(app.labels(), ["a", "b", "end"]);
    app.click("Fewer");
    assert_eq!(app.labels(), ["b", "end"]);
}

#[test]
fn a_button_can_remove_itself() {
    let app = start(
        "state items: [int] = [1, 2]
         window(\"Test\") {
           for item in items { button(\"Remove {item}\") { items.remove(0) } }
         }",
    );
    let first = app.fake.find(FakeKind::Button)[0];
    app.click("Remove 1");
    assert!(app.fake.is_removed(first));
    assert_eq!(app.fake.visible_texts(FakeKind::Button), ["Remove 2"]);
}

#[test]
fn writes_through_loop_variables_reach_state() {
    let app = start(
        "struct Item { count: int }
         state items: [Item] = [Item(count: 1), Item(count: 5)]
         fn bump_all() {
           for item in items { item.count += 10 }
         }
         window(\"Test\") {
           for item in items { label(\"{item.count}\") }
           button(\"Bump\") { bump_all() }
         }",
    );
    app.click("Bump");
    assert_eq!(app.labels(), ["11", "15"]);
}

#[test]
fn views_rebuild_when_their_arguments_change() {
    let app = start(
        "state n = 1
         view Badge(value: int) { label(\"#{value}\") }
         window(\"Test\") {
           Badge(value: n * 2)
           button(\"Next\") { n += 1 }
         }",
    );
    assert_eq!(app.labels(), ["#2"]);
    app.click("Next");
    assert_eq!(app.labels(), ["#4"]);
}

#[test]
fn values_are_copied_not_shared() {
    let app = start(
        "state a: [int] = [1]
         state b: [int] = []
         window(\"Test\") {
           button(\"Copy\") {
             b = a
             b.push(2)
           }
         }",
    );
    app.click("Copy");
    assert_eq!(app.state(0), Value::List(Rc::new(vec![Value::Int(1)])));
    assert_eq!(
        app.state(1),
        Value::List(Rc::new(vec![Value::Int(1), Value::Int(2)]))
    );
}

#[test]
fn runtime_errors_reach_the_error_handler() {
    let app = start(
        "state n = 0
         state items: [int] = []
         fn forever(x: int) -> int { return forever(x + 1) }
         window(\"Test\") {
           label(\"{n}\")
           button(\"Divide\") { n = 10 / n }
           button(\"Index\") { n = items[3] }
           button(\"Recurse\") { n = forever(0) }
         }",
    );
    app.click("Divide");
    app.click("Index");
    app.click("Recurse");
    assert_eq!(
        *app.errors.borrow(),
        [
            "can't divide by zero",
            "there is no item 3; the list has 0",
            "`forever` calls itself too deeply; does it ever stop?"
        ]
    );
    assert_eq!(app.labels(), ["0"], "failed actions leave the UI as it was");
}

#[test]
fn several_windows_open_together() {
    let app = start(
        "window(\"One\") { label(\"1\") }
         window(\"Two\") { label(\"2\") }",
    );
    assert!(app.fake.is_shown(0) && app.fake.is_shown(1));
    assert_eq!(app.fake.window_title(1), "Two");
}

#[test]
fn progress_bars_follow_their_value() {
    let app = start(
        "state percent = 40\nwindow(\"P\") {\n  progress(value: percent * 2)\n  progress()\n  button(\"More\") { percent += 20 }\n}",
    );
    let [counted, busy] = app.fake.find(FakeKind::Progress)[..] else {
        panic!("the window shows two progress bars");
    };
    assert_eq!(app.fake.value(counted), Some(80));
    assert_eq!(app.fake.value(busy), None);

    app.click("More");
    assert_eq!(app.fake.value(counted), Some(100), "it stays full past 100");
    assert_eq!(app.fake.value(busy), None);
}

#[test]
fn return_runs_only_a_text_fields_own_action() {
    let app = start(
        "state query = \"\"\nstate searched = \"\"\nstate typed = \"\"\nwindow(\"S\") {\n  text_field($query) { searched = query }\n  text_field($typed)\n  label(\"searched {searched}\")\n}",
    );
    let [search, plain] = app.fake.find(FakeKind::TextField)[..] else {
        panic!("the window shows two text fields");
    };
    app.fake.type_text(search, "cats");
    assert!(app.fake.submit(search));
    assert_eq!(app.labels(), ["searched cats"]);
    assert!(
        !app.fake.submit(plain),
        "Return in a field without an action is left to the primary button"
    );
}

#[test]
fn switches_sliders_and_pickers_edit_their_state() {
    let app = start(
        "state wifi = false\nstate volume = 3\nstate quality = \"Low\"\nstate qualities = [\"Low\", \"High\"]\nwindow(\"C\") {\n  switch(on: $wifi, title: \"Wi-Fi\")\n  slider($volume, min: 1, max: 5)\n  picker($quality, options: qualities)\n  label(\"{wifi} {volume} {quality}\")\n  button(\"More\") { qualities.push(\"Best\") }\n}",
    );
    let switch = app.only(FakeKind::Switch);
    let slider = app.only(FakeKind::Slider);
    let picker = app.only(FakeKind::Picker);
    assert_eq!(app.fake.text(switch), "Wi-Fi");
    assert_eq!(app.fake.range(slider), Some((1, 5)));
    assert_eq!(app.fake.value(slider), Some(3));
    assert_eq!(app.fake.text(picker), "Low");
    assert_eq!(app.fake.options(picker), ["Low", "High"]);

    app.fake.toggle(switch);
    app.fake.slide(slider, 5);
    app.fake.pick(picker, "High");
    assert_eq!(app.labels(), ["true 5 High"]);

    app.click("More");
    assert_eq!(app.fake.options(picker), ["Low", "High", "Best"]);
    assert_eq!(app.fake.text(picker), "High");
}
