//! The engine against a hand-written [`Logic`], with no language behind it: one `count`
//! state, a label showing it, a button adding to it, and a region with one row per count.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use super::*;
use crate::state::StateId;
use crate::style::{Axis, StackStyle};
use crate::testing::{FakeBackend, FakeKind};

const COUNT: StateId = StateId(0);
const NAME: StateId = StateId(1);

/// The row a region item shows, or nothing for the window's own content.
type Row = Option<i64>;

#[derive(Clone, Copy, Debug)]
enum Text {
    Count,
    Name,
    Row,
}

#[derive(Clone, Copy, Debug)]
struct NameBinding;

#[derive(Clone, Copy, Debug)]
struct AddOne;

#[derive(Debug)]
struct Rows;

struct Counter {
    count: Cell<i64>,
    name: RefCell<String>,
    changed: RefCell<StateSet>,
    row: Templates<Self>,
}

impl Counter {
    fn changing(&self, state: StateId) {
        self.changed.borrow_mut().insert(state);
    }
}

impl Logic for Counter {
    type Frame = Row;
    type Text = Text;
    type Binding = NameBinding;
    type Action = AddOne;
    type Region = Rows;
    type Error = String;

    fn text(&self, text: &Text, row: &Row) -> Result<String, String> {
        Ok(match (text, row) {
            (Text::Count, _) => format!("count {}", self.count.get()),
            (Text::Name, _) => format!("hello {}", self.name.borrow()),
            (Text::Row, Some(row)) => format!("row {row}"),
            (Text::Row, None) => return Err("a row label outside a row".to_owned()),
        })
    }

    fn read_text(&self, NameBinding: &NameBinding, _: &Row) -> Result<String, String> {
        Ok(self.name.borrow().clone())
    }

    fn read_checked(&self, NameBinding: &NameBinding, _: &Row) -> Result<bool, String> {
        Err("the name isn't a bool".to_owned())
    }

    fn write_text(&self, NameBinding: &NameBinding, _: &Row, text: &str) -> Result<(), String> {
        text.clone_into(&mut self.name.borrow_mut());
        self.changing(NAME);
        Ok(())
    }

    fn write_checked(&self, NameBinding: &NameBinding, _: &Row, _: bool) -> Result<(), String> {
        Err("the name isn't a bool".to_owned())
    }

    fn run(&self, AddOne: &AddOne, _: &Row) -> Result<(), String> {
        if self.count.get() == 3 {
            return Err("three is enough".to_owned());
        }
        self.count.set(self.count.get() + 1);
        self.changing(COUNT);
        Ok(())
    }

    fn expand(&self, Rows: &Rows, _: &Rc<Row>) -> Result<Vec<Shown<Self>>, String> {
        Ok((0..self.count.get())
            .map(|row| Shown {
                templates: Rc::clone(&self.row),
                frame: Rc::new(Some(row)),
            })
            .collect())
    }

    fn take_changed(&self) -> StateSet {
        self.changed.take()
    }
}

fn deps(states: &[StateId]) -> StateSet {
    let mut deps = StateSet::default();
    for state in states {
        deps.insert(*state);
    }
    deps
}

fn label(text: Text, states: &[StateId]) -> Template<Counter> {
    Template::Widget {
        widget: Widget::Label {
            text,
            style: crate::LabelStyle::default(),
        },
        deps: deps(states),
    }
}

fn window(title: &str) -> WindowTemplate<Counter> {
    WindowTemplate {
        backdrop: crate::Backdrop::Standard,
        title: title.to_owned(),
        size: Size {
            width: 300.0,
            height: 200.0,
        },
        style: StackStyle::new(Axis::Vertical),
        children: [
            label(Text::Count, &[COUNT]),
            Template::Region {
                region: Rows,
                deps: deps(&[COUNT]),
            },
            label(Text::Name, &[NAME]),
            Template::Widget {
                widget: Widget::TextField {
                    value: NameBinding,
                    placeholder: "Name".to_owned(),
                },
                deps: deps(&[NAME]),
            },
            Template::Widget {
                widget: Widget::Button {
                    title: Text::Count,
                    style: crate::ButtonStyle::Normal,
                    icon: crate::Icon::None,
                    action: AddOne,
                },
                deps: deps(&[COUNT]),
            },
        ]
        .into(),
        frame: None,
    }
}

fn counter(_: &Ui<FakeBackend>, _: Weak<Engine<Counter, FakeBackend>>) -> Counter {
    Counter {
        count: Cell::new(0),
        name: RefCell::new(String::new()),
        changed: RefCell::new(StateSet::default()),
        row: [label(Text::Row, &[])].into(),
    }
}

struct Running {
    engine: Rc<Engine<Counter, FakeBackend>>,
    fake: FakeBackend,
    errors: Rc<RefCell<Vec<String>>>,
}

fn start(windows: Vec<WindowTemplate<Counter>>) -> Running {
    let fake = FakeBackend::default();
    let errors = Rc::new(RefCell::new(Vec::new()));
    let on_error = Rc::new({
        let errors = Rc::clone(&errors);
        move |error| errors.borrow_mut().push(error)
    });
    let engine = Engine::start(Ui::new(fake.clone()), windows, on_error, counter)
        .expect("the test app starts");
    Running {
        engine,
        fake,
        errors,
    }
}

impl Running {
    fn labels(&self) -> Vec<String> {
        self.fake.visible_texts(FakeKind::Label)
    }

    fn click(&self) {
        let button = self.fake.find(FakeKind::Button)[0];
        self.fake.click(button);
    }
}

#[test]
fn updates_widgets_and_rebuilds_regions_in_place() {
    let app = start(vec![window("Counter")]);
    assert!(app.fake.is_shown(0));
    assert_eq!(app.labels(), ["count 0", "hello "]);

    app.click();
    app.click();
    assert_eq!(app.labels(), ["count 2", "row 0", "row 1", "hello "]);
    assert_eq!(app.fake.visible_texts(FakeKind::Button), ["count 2"]);

    let field = app.fake.find(FakeKind::TextField)[0];
    app.fake.type_text(field, "Ada");
    assert_eq!(app.labels(), ["count 2", "row 0", "row 1", "hello Ada"]);
}

#[test]
fn errors_from_events_go_to_the_error_handler() {
    let app = start(vec![window("Counter")]);
    for _ in 0..4 {
        app.click();
    }
    assert_eq!(*app.errors.borrow(), ["three is enough"]);
    assert_eq!(app.labels()[0], "count 3");
}

#[test]
fn reloading_reuses_windows_in_order_and_closes_the_rest() {
    let app = start(vec![window("One"), window("Two")]);
    app.click();

    let next = app
        .engine
        .reload(vec![window("Renamed")], counter)
        .expect("the reload builds");

    assert_eq!(app.fake.window_count(), 2);
    assert_eq!(app.fake.window_title(0), "Renamed");
    assert!(app.fake.is_closed(1));
    assert_eq!(next.logic().count.get(), 0, "the new logic starts over");
    assert_eq!(app.labels(), ["count 0", "hello "]);
}

#[test]
fn a_failed_reload_leaves_the_app_as_it_was() {
    let app = start(vec![window("Counter")]);
    let mut broken = window("Broken");
    broken.children = [label(Text::Row, &[])].into();

    let error = app
        .engine
        .reload(vec![broken], counter)
        .expect_err("a row label outside a row fails");

    assert_eq!(error, "a row label outside a row");
    assert_eq!(app.fake.window_title(0), "Counter");
    app.click();
    assert_eq!(app.labels(), ["count 1", "row 0", "hello "]);
}
