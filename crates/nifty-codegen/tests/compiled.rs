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
    /// Toggles the checkbox at this position among the visible ones.
    Toggle(usize),
    /// Plays the UI thread, which shows the dialogs asked for and applies their answers.
    Wake,
    /// Picks the nav item with this title.
    Pick(&'a str),
    /// Closes the dialog shown at this position, choosing these paths, or cancelling when
    /// there are none.
    Answer(usize, &'a [&'a str]),
    /// Toggles the switch at this position among the visible ones.
    Switch(usize),
    /// Drags the slider at this position among the visible ones to this value.
    Slide(usize, i64),
    /// Picks this option from the picker at this position among the visible ones.
    Choose(usize, &'a str),
}

impl Running {
    /// Everything visible, and every error so far.
    fn snapshot(&self) -> String {
        let mut lines = Vec::new();
        for kind in [FakeKind::Label, FakeKind::Button, FakeKind::TextField] {
            lines.push(format!("{kind:?}: {:?}", self.fake.visible_texts(kind)));
        }
        let checkboxes: Vec<_> = self
            .fake
            .find(FakeKind::Checkbox)
            .into_iter()
            .map(|checkbox| (self.fake.text(checkbox), self.fake.is_checked(checkbox)))
            .collect();
        lines.push(format!("Checkbox: {checkboxes:?}"));
        let nav_items: Vec<_> = self
            .fake
            .find(FakeKind::NavItem)
            .into_iter()
            .map(|item| {
                (
                    self.fake.text(item),
                    self.fake.icon(item),
                    self.fake.badge(item),
                    self.fake.is_checked(item),
                )
            })
            .collect();
        lines.push(format!("NavItem: {nav_items:?}"));
        let icons: Vec<_> = self
            .fake
            .find(FakeKind::Button)
            .into_iter()
            .map(|button| (self.fake.style(button), self.fake.icon(button)))
            .collect();
        lines.push(format!("Button looks: {icons:?}"));
        let bars: Vec<_> = self
            .fake
            .find(FakeKind::Progress)
            .into_iter()
            .map(|bar| self.fake.value(bar))
            .collect();
        lines.push(format!("Progress: {bars:?}"));
        let switches: Vec<_> = self
            .fake
            .find(FakeKind::Switch)
            .into_iter()
            .map(|switch| (self.fake.text(switch), self.fake.is_checked(switch)))
            .collect();
        lines.push(format!("Switch: {switches:?}"));
        let sliders: Vec<_> = self
            .fake
            .find(FakeKind::Slider)
            .into_iter()
            .map(|slider| (self.fake.value(slider), self.fake.range(slider)))
            .collect();
        lines.push(format!("Slider: {sliders:?}"));
        let pickers: Vec<_> = self
            .fake
            .find(FakeKind::Picker)
            .into_iter()
            .map(|picker| (self.fake.text(picker), self.fake.options(picker)))
            .collect();
        lines.push(format!("Picker: {pickers:?}"));
        let scrolls: Vec<_> = self
            .fake
            .find(FakeKind::Scroll)
            .into_iter()
            .map(|scroll| (self.fake.frame(scroll), self.fake.scroll_content(scroll)))
            .collect();
        lines.push(format!("Scroll: {scrolls:?}"));
        let surfaces: Vec<_> = self
            .fake
            .find(FakeKind::Surface)
            .into_iter()
            .map(|surface| (self.fake.style(surface), self.fake.frame(surface)))
            .collect();
        lines.push(format!("Surface: {surfaces:?}"));
        let windows: Vec<_> = (0..self.fake.window_count())
            .map(|window| (self.fake.window_title(window), self.fake.backdrop(window)))
            .collect();
        lines.push(format!("Windows: {windows:?}"));
        lines.push(format!("dialogs: {:?}", self.fake.dialogs()));
        lines.push(format!("errors: {:?}", self.errors.borrow()));
        lines.join("\n")
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
            Action::Toggle(index) => self.fake.toggle(self.fake.find(FakeKind::Checkbox)[index]),
            Action::Switch(index) => self.fake.toggle(self.fake.find(FakeKind::Switch)[index]),
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
            Action::Toggle(0),
            Action::Pick("Inbox"),
            Action::Click("Add"),
            Action::Switch(0),
            Action::Slide(0, 9),
            Action::Choose(0, "Slow"),
        ],
    );
    for shown in [
        "Volume 9",
        r#"Switch: [("Notify me", false)]"#,
        r#"Picker: [("Slow", ["Fast", "Balanced", "Slow"])]"#,
    ] {
        assert!(end.contains(shown), "{end}");
    }
    assert!(end.contains(r#"("Solid backdrop", Solid)"#), "{end}");
    for surface in ["Sidebar", "Content", "Card"] {
        assert!(end.contains(&format!("Some(\"{surface}\")")), "{end}");
    }
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
  for word in words {
    if word.len() > limit {
      return word
    }
  }
  return "none"
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
        ],
    );
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
    app.act(Action::Type(0, &url));
    app.act(Action::Type(1, &folder.display().to_string()));
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
