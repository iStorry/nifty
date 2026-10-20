//! Compiles programs, loads them in-process, and drives them on the fake backend next to
//! the interpreter running the same program: after every step, both must show the same
//! thing and report the same errors.

use std::cell::RefCell;
use std::fs;
use std::rc::Rc;

use cranelift_jit::{JITBuilder, JITModule};
use nifty_codegen::{Emitter, compile, host_isa};
use nifty_download::testing::{scratch_folder, serve_once};
use nifty_rt::{AppError, Compiled, ProgramDesc, symbols};
use nifty_runtime::testing::{FakeBackend, FakeKind};
use nifty_runtime::{Engine, Picked, Ui};

mod support;

use support::{check, locate};

/// Compiles `source` into memory and returns its descriptor, which lives as long as the
/// test process, as the compiled code does.
fn load(source: &str) -> &'static ProgramDesc {
    let program = check(source);
    let mut builder = JITBuilder::with_isa(
        host_isa(false).expect("Cranelift supports this system"),
        cranelift_module::default_libcall_names(),
    );
    for (name, address) in symbols::addresses() {
        builder.symbol(name, address);
    }
    let mut emitter = Emitter::new(JITModule::new(builder));
    let descriptor = compile(&mut emitter, &program, &|span| locate(source, span))
        .unwrap_or_else(|error| panic!("the program compiles: {error}"));
    let mut module = emitter.into_module();
    module
        .finalize_definitions()
        .expect("the compiled code loads");
    let (address, _) = module.get_finalized_data(descriptor);
    Box::leak(Box::new(module));
    assert_eq!(address.align_offset(align_of::<ProgramDesc>()), 0);
    #[expect(
        clippy::cast_ptr_alignment,
        reason = "Cranelift hands data back as bytes; the compiler aligns it, as checked above"
    )]
    let descriptor = address.cast::<ProgramDesc>();
    // SAFETY: the compiler laid out a `ProgramDesc` at this address, and the module that
    // holds it was just leaked, so it lives for the rest of the process.
    unsafe { &*descriptor }
}

/// One way of running an app: on its own fake backend, recording its errors.
struct Running {
    fake: FakeBackend,
    errors: Rc<RefCell<Vec<String>>>,
    /// Keeps the app alive while the test drives it.
    _app: Box<dyn std::any::Any>,
}

fn interpreted(source: &str) -> Running {
    let fake = FakeBackend::default();
    let errors = Rc::new(RefCell::new(Vec::new()));
    let on_error = Rc::new({
        let errors = Rc::clone(&errors);
        move |error: nifty_interp::RuntimeError| errors.borrow_mut().push(error.message)
    });
    let app = nifty_interp::App::start(check(source), Ui::new(fake.clone()), on_error)
        .unwrap_or_else(|error| panic!("the interpreted app starts: {}", error.message));
    Running {
        fake,
        errors,
        _app: Box::new(app),
    }
}

/// The compiled app, and its engine, for tests that reach into its logic.
fn compiled_engine(source: &str) -> (Running, Rc<Engine<Compiled, FakeBackend>>) {
    let fake = FakeBackend::default();
    let errors = Rc::new(RefCell::new(Vec::new()));
    let on_error = Rc::new({
        let errors = Rc::clone(&errors);
        move |error: AppError| errors.borrow_mut().push(error.message)
    });
    let engine = nifty_rt::prepare(load(source))
        .and_then(|prepared| prepared.show(Ui::new(fake.clone()), on_error))
        .unwrap_or_else(|error| panic!("the compiled app starts: {error}"));
    let running = Running {
        fake,
        errors,
        _app: Box::new(Rc::clone(&engine)),
    };
    (running, engine)
}

fn compiled(source: &str) -> Running {
    compiled_engine(source).0
}

#[derive(Clone, Copy, Debug)]
enum Action<'a> {
    Click(&'a str),
    /// Types into the text field at this position among the visible ones.
    Type(usize, &'a str),
    /// Types into the text field made last, such as one in the window opened last.
    TypeInNewest(&'a str),
    /// Toggles the checkbox at this position among the visible ones.
    Toggle(usize),
    /// Plays the UI thread, which shows the dialogs asked for and applies their answers.
    Wake,
    /// Picks the nav item with this title.
    Pick(&'a str),
    /// Closes the dialog shown at this position, choosing these paths, or cancelling when
    /// there are none.
    Answer(usize, &'a [&'a str]),
    /// Chooses the menu item at the end of this path of titles.
    Menu(&'a [&'a str]),
    /// Presses Return in the text field at this position among the visible ones.
    Submit(usize),
    /// Toggles the switch at this position among the visible ones.
    Switch(usize),
    /// Drags the slider at this position among the visible ones to this value.
    Slide(usize, i64),
    /// Picks this option from the picker at this position among the visible ones.
    Choose(usize, &'a str),
    /// Chooses the item at the end of this path from the context menu at this position.
    ContextMenu(usize, &'a [&'a str]),
    /// Asks to close the window opened at this position, as its close button does.
    CloseWindow(usize),
    /// Drops these files and links on the drop target at this position.
    Drop(usize, &'a [&'a str]),
}

impl Running {
    /// Everything visible, and every error so far.
    fn snapshot(&self) -> String {
        let fake = &self.fake;
        let mut lines = Vec::new();
        for kind in [FakeKind::Label, FakeKind::Button, FakeKind::TextField] {
            lines.push(format!("{kind:?}: {:?}", fake.visible_texts(kind)));
        }
        lines.extend([
            self.each("Checkbox", FakeKind::Checkbox, |w| {
                (fake.text(w), fake.is_checked(w))
            }),
            self.each("NavItem", FakeKind::NavItem, |w| {
                (
                    fake.text(w),
                    fake.icon(w),
                    fake.badge(w),
                    fake.is_checked(w),
                )
            }),
            self.each("Button looks", FakeKind::Button, |w| {
                (fake.style(w), fake.icon(w), fake.selection(w))
            }),
            self.each("Progress", FakeKind::Progress, |w| fake.value(w)),
            self.each("Image", FakeKind::Image, |w| (fake.text(w), fake.frame(w))),
            self.each("TextField lines", FakeKind::TextField, |w| fake.lines(w)),
            self.each("Switch", FakeKind::Switch, |w| {
                (fake.text(w), fake.is_checked(w))
            }),
            self.each("Slider", FakeKind::Slider, |w| {
                (fake.value(w), fake.range(w))
            }),
            self.each("Picker", FakeKind::Picker, |w| {
                (fake.text(w), fake.options(w))
            }),
            self.each("Scroll", FakeKind::Scroll, |w| {
                (fake.frame(w), fake.scroll_content(w))
            }),
            self.each("Surface", FakeKind::Surface, |w| {
                (fake.style(w), fake.frame(w))
            }),
        ]);
        let windows: Vec<_> = (0..fake.window_count())
            .map(|window| {
                (
                    fake.window_title(window),
                    fake.backdrop(window),
                    fake.is_closed(window),
                )
            })
            .collect();
        lines.push(format!("Windows: {windows:?}"));
        lines.push(format!("menus: {:?}", fake.menus()));
        lines.push(format!("dialogs: {:?}", fake.dialogs()));
        lines.push(format!("errors: {:?}", self.errors.borrow()));
        lines.join("\n")
    }

    /// `name`, then what `show` says of each live widget of `kind`, in reading order.
    fn each<T: std::fmt::Debug>(
        &self,
        name: &str,
        kind: FakeKind,
        show: impl Fn(usize) -> T,
    ) -> String {
        let shown: Vec<T> = self.fake.find(kind).into_iter().map(show).collect();
        format!("{name}: {shown:?}")
    }

    fn act(&self, action: Action<'_>) {
        match action {
            Action::Click(title) => {
                let button = self
                    .fake
                    .find(FakeKind::Button)
                    .into_iter()
                    .find(|button| self.fake.text(*button) == title)
                    .unwrap_or_else(|| panic!("no button titled {title:?}"));
                self.fake.click(button);
            }
            Action::Type(index, text) => {
                self.fake
                    .type_text(self.fake.find(FakeKind::TextField)[index], text);
            }
            Action::TypeInNewest(text) => {
                let newest = self.fake.find(FakeKind::TextField).into_iter().max();
                self.fake
                    .type_text(newest.expect("a text field is showing"), text);
            }
            Action::Toggle(index) => self.fake.toggle(self.fake.find(FakeKind::Checkbox)[index]),
            Action::Switch(index) => self.fake.toggle(self.fake.find(FakeKind::Switch)[index]),
            Action::Menu(path) => self.fake.choose_menu_item(path),
            Action::CloseWindow(window) => self.fake.request_close(window),
            Action::Drop(index, items) => self
                .fake
                .drop_items(self.fake.find(FakeKind::DropTarget)[index], items),
            Action::ContextMenu(index, path) => self
                .fake
                .choose_context_item(self.fake.find(FakeKind::ContextMenu)[index], path),
            Action::Submit(index) => {
                let field = self.fake.find(FakeKind::TextField)[index];
                assert!(self.fake.submit(field), "the field has an action");
            }
            Action::Slide(index, value) => {
                self.fake
                    .slide(self.fake.find(FakeKind::Slider)[index], value);
            }
            Action::Choose(index, option) => {
                self.fake
                    .pick(self.fake.find(FakeKind::Picker)[index], option);
            }
            Action::Pick(title) => {
                let item = self
                    .fake
                    .find(FakeKind::NavItem)
                    .into_iter()
                    .find(|item| self.fake.text(*item) == title)
                    .unwrap_or_else(|| panic!("no nav item titled {title:?}"));
                self.fake.click(item);
            }
            Action::Wake => self.fake.run_pending_wakes(),
            Action::Answer(index, paths) => {
                let picked = if paths.is_empty() {
                    Picked::Cancelled
                } else {
                    Picked::Chosen(paths.iter().map(|path| (*path).to_owned()).collect())
                };
                self.fake.close_dialog(index, picked);
                self.fake.run_pending_wakes();
            }
        }
    }
}

/// Runs `source` both ways through `actions`, comparing after each, and returns what the
/// compiled app shows at the end.
fn same_both_ways(source: &str, actions: &[Action<'_>]) -> String {
    let (interpreter, machine_code) = (interpreted(source), compiled(source));
    assert_eq!(
        machine_code.snapshot(),
        interpreter.snapshot(),
        "at the start"
    );
    for action in actions {
        interpreter.act(*action);
        machine_code.act(*action);
        assert_eq!(
            machine_code.snapshot(),
            interpreter.snapshot(),
            "after {action:?}"
        );
    }
    machine_code.snapshot()
}

#[test]
fn the_counter_example() {
    let end = same_both_ways(
        include_str!("../../../fixtures/counter.nf"),
        &[
            Action::Click("+"),
            Action::Click("+"),
            Action::Click("−"),
            Action::Click("−"),
            Action::Click("−"),
        ],
    );
    assert!(end.contains("Clicked -1 times"), "{end}");
}

#[test]
fn the_todos_example() {
    let end = same_both_ways(
        include_str!("../../../fixtures/todos.nf"),
        &[
            Action::Type(0, "Milk"),
            Action::Click("Add"),
            Action::Type(0, "Bread"),
            Action::Click("Add"),
            Action::Toggle(0),
            Action::Toggle(1),
            Action::Toggle(0),
        ],
    );
    assert!(end.contains("1 left"), "{end}");
}

#[test]
fn the_gallery_example() {
    let end = same_both_ways(
        include_str!("../../../fixtures/gallery.nf"),
        &[
            Action::Click("Normal"),
            Action::Click("Primary"),
            Action::Click("Reset"),
            Action::Type(0, "Ada"),
            Action::Type(1, "Two\nlines"),
            Action::Toggle(0),
            Action::Pick("Inbox"),
            Action::Click("Add"),
            Action::Switch(0),
            Action::Slide(0, 9),
            Action::Choose(0, "Slow"),
            Action::Click("Active"),
            Action::Menu(&["Gallery", "Click"]),
            Action::Menu(&["Gallery", "Filter", "Starred"]),
            Action::ContextMenu(0, &["Add a Click"]),
            Action::Click("Choose image…"),
            Action::Wake,
            Action::Answer(0, &["/photos/cat.png"]),
        ],
    );
    for shown in [
        "Volume 9",
        "Clicked 3 times",
        "Showing starred",
        "shortcut: Some(Shortcut { key: K, shift: true, alt: false })",
        r#"Image: [("/photos/cat.png", Some(Frame"#,
        "size: Size { width: 64.0, height: 64.0 } }))]",
        r#"(Some("Icon"), Star, Some(true))"#,
        r#"Switch: [("Notify me", false)]"#,
        r#"Picker: [("Slow", ["Fast", "Balanced", "Slow"])]"#,
    ] {
        assert!(end.contains(shown), "{end}");
    }
    assert!(end.contains(r#"("Solid backdrop", Solid, false)"#), "{end}");
    for surface in ["Sidebar", "Content", "Card"] {
        assert!(end.contains(&format!("Some(\"{surface}\")")), "{end}");
    }
}

#[test]
fn return_runs_a_text_fields_action() {
    let end = same_both_ways(
        "state draft = \"\"\nstate items: [string] = []\nwindow(\"Notes\") {\n  text_field($draft) {\n    items.push(draft)\n    draft = \"\"\n  }\n  for item in items {\n    label(item)\n  }\n}",
        &[
            Action::Type(0, "milk"),
            Action::Submit(0),
            Action::Type(0, "eggs"),
            Action::Submit(0),
        ],
    );
    assert!(end.contains(r#"Label: ["milk", "eggs"]"#), "{end}");
    assert!(end.contains(r#"TextField: [""]"#), "{end}");
}

#[test]
fn drops_reach_state_through_a_rows_alias() {
    let end = same_both_ways(
        "struct Row {\n  name: string\n  files: [string]\n}\nstate rows = [Row(name: \"a\", files: []), Row(name: \"b\", files: [])]\nstate drops = 0\nwindow(\"Drops\") {\n  for row in rows {\n    row() {\n      drop_target($row.files) { drops += 1 }\n      label(\"{row.name}: {row.files.len()}\")\n    }\n  }\n  label(\"{drops} drops\")\n}",
        &[
            Action::Drop(1, &["/Users/ada/a.png", "https://example.com/b.zip"]),
            Action::Drop(0, &["/Users/ada/c.png"]),
        ],
    );
    assert!(
        end.contains(r#"Label: ["a: 1", "b: 2", "2 drops"]"#),
        "{end}"
    );
}

#[test]
fn code_opens_and_closes_named_windows() {
    let end = same_both_ways(
        "state name = \"Ada\"\nwindow main(\"Main\") {\n  button(\"Settings\") { open_window(settings) }\n}\nwindow settings(\"Settings\", open: false) {\n  label(\"Name: {name}\")\n  button(\"Done\") { close_window(settings) }\n}",
        &[
            Action::Click("Settings"),
            Action::Click("Settings"),
            Action::Click("Done"),
            Action::Click("Settings"),
            Action::CloseWindow(2),
        ],
    );
    assert!(
        end.contains(
            r#"Windows: [("Main", Standard, false), ("Settings", Standard, true), ("Settings", Standard, true)]"#
        ),
        "{end}"
    );
}

#[test]
fn a_rows_context_menu_acts_on_its_own_item() {
    let end = same_both_ways(
        "state names = [\"a\", \"b\", \"c\"]\nstate chosen = \"\"\nwindow(\"Rows\") {\n  for name in names {\n    row() {\n      context_menu() {\n        menu(\"Choose\") {\n          menu_item(\"This\") { chosen = name }\n        }\n        divider()\n        menu_item(\"Shout\") { chosen = \"{name}!\" }\n      }\n      label(name)\n    }\n  }\n  label(\"chose {chosen}\")\n}",
        &[
            Action::ContextMenu(1, &["Choose", "This"]),
            Action::ContextMenu(2, &["Shout"]),
        ],
    );
    assert!(
        end.contains(r#"Label: ["a", "b", "c", "chose c!"]"#),
        "{end}"
    );
}

#[test]
fn the_sidebar_example() {
    let end = same_both_ways(
        include_str!("../../../fixtures/sidebar.nf"),
        &[
            Action::Pick("Sent"),
            Action::Pick("Archive"),
            Action::Click("Mark all read"),
        ],
    );
    assert!(end.contains("Welcome to archive"), "{end}");
    assert!(end.contains(r#"("Archive", Archive, "", true)"#), "{end}");
}

const LANGUAGE: &str = r#"
struct Item {
  name: string
  tags: [string]
  count: int
}

state items: [Item] = [Item(name: "a", tags: ["x"], count: 1)]
state ratio = 1.5
state log = ""
state big = 9223372036854775807
state zero = 0
state words: [string] = ["pear", "apple"]

fn fib(n: int) -> int {
  if n < 2 {
    return n
  }
  return fib(n - 1) + fib(n - 2)
}

fn forever(n: int) -> int {
  return forever(n + 1)
}

fn describe(item: Item) -> string {
  return "{item.name}:{item.count}:{item.tags.len()}"
}

fn bump_all() {
  for item in items {
    item.count += 1
    item.tags.push("t{item.count}")
  }
}

fn total() -> int {
  let sum = 0
  for item in items {
    sum += item.count
  }
  return sum
}

fn first_long(limit: int) -> string {
  for index, word in words {
    if word.len() > limit {
      return "{index}:{word}"
    }
  }
  return "none"
}

fn spelled() -> string {
  let text = ""
  for index, word in ["x", "y"] {
    text = text + "{index}{word}"
  }
  for n in zero - 2..zero + 2 {
    text = text + " {n}"
  }
  return text
}

view Row(item: Item, position: int) {
  row() {
    label("{position}. {describe(item)}")
  }
}

window("Language") {
  label("fib {fib(10)} total {total()} long {first_long(4)}")
  label("ratio {ratio} {ratio * 2.0} {ratio / 4.0} {ratio % 1.0} {-ratio} {ratio > 1.0}")
  label("{items.len()} items, first tags {items[0].tags.len()}")
  label("log: {log}")
  label("{words[0] < words[1]} {words == words} {items.is_empty()} {words[0] + words[1]}")
  for item in items {
    label(item.name)
    for tag in item.tags {
      label("tag {tag}")
    }
    button("rename {item.name}") { item.name = item.name + "!" }
  }
  for word in words {
    label("word {word}")
  }
  label(spelled())
  for index, letter in ["p", "q"] {
    label("letter {index}{letter}")
  }
  for index, item in items {
    button("drop {index}") { items.remove(index) }
  }
  for n in 0..items.len() {
    label("row {n}")
  }
  if items.len() > 2 {
    label("many")
  } else if items.len() > 1 {
    label("two")
  } else {
    label("one")
  }
  Row(item: Item(name: "view", tags: words, count: total()), position: 1)
  button("add") { items.push(Item(name: "b{items.len()}", tags: [], count: 0)) }
  button("bump") { bump_all() }
  button("remove first") { items.remove(0) }
  button("copy") {
    let first = items[0]
    first.name = "changed"
    log = "{first.name} vs {items[0].name}"
  }
  button("words") {
    words.push("kiwi")
    words[0] = "plum"
    ratio *= 2.0
  }
  button("grow") {
    for n in 0..items.len() {
      items.push(Item(name: "g{n}", tags: [], count: n))
    }
  }
  button("divide") { log = "{10 / zero}" }
  button("overflow") { log = "{big + 1}" }
  button("deep") { log = "{forever(0)}" }
  button("missing") { log = "{items[5].name}" }
  button("negative") { log = "{items[zero - 1].name}" }
}
"#;

#[test]
fn the_whole_language() {
    same_both_ways(
        LANGUAGE,
        &[
            Action::Click("add"),
            Action::Click("bump"),
            Action::Click("rename a"),
            Action::Click("bump"),
            Action::Click("copy"),
            Action::Click("words"),
            Action::Click("add"),
            Action::Click("remove first"),
            Action::Click("rename b1"),
            Action::Click("grow"),
            Action::Click("drop 1"),
        ],
    );
}

#[test]
fn the_lineup_example() {
    let end = same_both_ways(
        include_str!("../../../fixtures/lineup.nf"),
        &[
            Action::Click("Remove Grace"),
            Action::Type(0, "Ada"),
            Action::Click("Add"),
            Action::Type(0, "Mae"),
            Action::Click("Add"),
            Action::Click("More"),
            Action::Click("Fewer"),
            Action::Click("Fewer"),
            Action::Click("Fewer"),
            Action::Click("Fewer"),
            Action::Click("Fewer"),
        ],
    );
    assert!(end.contains(r#""1. Ada", "2. Linus", "3. Mae""#), "{end}");
    assert!(end.contains(r#""○○○○○""#), "{end}");
    assert!(!end.contains('★'), "{end}");
}

#[test]
fn failures_match_the_interpreter() {
    for button in ["divide", "overflow", "deep", "missing", "negative"] {
        let end = same_both_ways(LANGUAGE, &[Action::Click(button)]);
        assert!(
            end.contains("errors: [\""),
            "{button} reports an error: {end}"
        );
    }
}

#[test]
fn the_files_example() {
    let end = same_both_ways(
        include_str!("../../../fixtures/files.nf"),
        &[
            Action::Click("Choose photo…"),
            Action::Wake,
            Action::Answer(0, &["/photos/cat.png"]),
            Action::Click("Attach files…"),
            Action::Wake,
            Action::Answer(1, &["/a.txt", "/b.txt"]),
            Action::Click("Choose folder…"),
            Action::Wake,
            Action::Answer(2, &[]),
            Action::Click("Export…"),
            Action::Wake,
            Action::Answer(3, &["/out/export.csv"]),
        ],
    );
    for shown in [
        "/photos/cat.png",
        "2 attached",
        "/out/export.csv",
        "errors: []",
    ] {
        assert!(end.contains(shown), "{end}");
    }
}

#[test]
fn the_downloader_example() {
    let folder = scratch_folder("codegen-downloader");
    let url = format!("{}/report.pdf", serve_once("200 OK", "%PDF"));
    let (app, engine) = compiled_engine(include_str!("../../../fixtures/downloader.nf"));
    app.act(Action::Menu(&["Downloader", "Settings…"]));
    app.act(Action::TypeInNewest(&folder.display().to_string()));
    app.act(Action::Click("Done"));
    app.act(Action::Type(0, &url));
    app.act(Action::Click("Download"));

    engine.logic().wait_for_downloads();
    app.fake.run_pending_wakes();

    let saved = folder.join("report.pdf");
    let labels = app.fake.visible_texts(FakeKind::Label);
    assert!(
        labels.contains(&format!("Saved to {}", saved.display())),
        "{labels:?}"
    );
    let bars = app.fake.find(FakeKind::Progress);
    assert_eq!(bars.len(), 1);
    assert_eq!(app.fake.value(bars[0]), Some(100));
    assert_eq!(
        fs::read_to_string(&saved).expect("read the saved file"),
        "%PDF"
    );
    fs::remove_dir_all(&folder).expect("remove scratch folder");
}
