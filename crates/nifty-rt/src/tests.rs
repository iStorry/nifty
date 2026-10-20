//! A program written by hand the way `nf build` compiles one, run on the fake backend.
//! It stands for:
//!
//! ```nifty
//! state count = 0
//! state names: [string] = ["a"]
//! state draft = ""
//! state done = false
//!
//! view Greeting(name: string) { label("Hi {name}") }
//!
//! window("Test") {
//!   label("Clicked {count} times")
//!   if count > 1 { label("many") } else { label("few") }
//!   for name in names { label(name) }
//!   Greeting(name: draft)
//!   text_field(draft)
//!   checkbox(checked: done, title: "Done")
//!   button("+") { count += 1 }
//!   button("Add") { names.push("b") }
//!   button("Break") { count = count / 0 }
//! }
//! ```

use std::cell::RefCell;
use std::rc::Rc;

use nifty_runtime::testing::{FakeBackend, FakeKind};
use nifty_runtime::{ButtonStyle, Engine, Key, Shortcut, Ui};

use super::*;
use crate::abi::{
    nf_fail_divide_by_zero, nf_list_get, nf_list_len, nf_list_new, nf_list_push, nf_retain,
    nf_state, nf_state_mut, nf_store, nf_string_new, nf_text_finish, nf_text_new, nf_text_push,
    nf_text_push_str,
};
use crate::show::{nf_show_item, nf_show_same, nf_show_view};
use crate::value::{LIST_OF_STRINGS, int, int_slot};

const COUNT: u32 = 0;
const NAMES: u32 = 1;
const DRAFT: u32 = 2;
const DONE: u32 = 3;
/// Where `count / 0` is, in `SPANS`.
const DIVISION: u32 = 0;

/// # Safety
/// `bytes` lives as long as the string's use.
unsafe fn literal(text: &str) -> Slot {
    // SAFETY: `text` is readable for its length.
    unsafe { nf_string_new(text.as_ptr(), text.len()) }
}

/// Retains a borrowed value, as compiled code does to keep one.
fn kept(slot: Slot, ty: &TypeDesc) -> Slot {
    nf_retain(slot, ty);
    slot
}

unsafe extern "C" fn zero(_: *const Context, out: *mut Slot) -> bool {
    // SAFETY: `out` is writable.
    unsafe { out.write(int_slot(0)) };
    true
}

unsafe extern "C" fn just_a(_: *const Context, out: *mut Slot) -> bool {
    // SAFETY: the item is an owned string, and `out` is writable.
    unsafe {
        let items = [literal("a")];
        out.write(nf_list_new(&LIST_OF_STRINGS, items.as_ptr(), items.len()));
    }
    true
}

unsafe extern "C" fn empty_string(_: *const Context, out: *mut Slot) -> bool {
    // SAFETY: `out` is writable.
    unsafe { out.write(literal("")) };
    true
}

unsafe extern "C" fn no(_: *const Context, out: *mut Slot) -> bool {
    // SAFETY: `out` is writable.
    unsafe { out.write(0) };
    true
}

unsafe extern "C" fn clicked(context: *const Context, _: *const Slot, out: *mut Slot) -> bool {
    // SAFETY: the context is live, the builder is used once, and `out` is writable.
    unsafe {
        let text = nf_text_new();
        nf_text_push_str(text, "Clicked ".as_ptr(), "Clicked ".len());
        nf_text_push(text, nf_state(context, COUNT), &INT);
        nf_text_push_str(text, " times".as_ptr(), " times".len());
        out.write(nf_text_finish(text));
    }
    true
}

macro_rules! constant_text {
    ($name:ident, $text:literal) => {
        unsafe extern "C" fn $name(_: *const Context, _: *const Slot, out: *mut Slot) -> bool {
            // SAFETY: `out` is writable.
            unsafe { out.write(literal($text)) };
            true
        }
    };
}

constant_text!(many, "many");
constant_text!(few, "few");
constant_text!(plus, "+");
constant_text!(add, "Add");
constant_text!(break_title, "Break");
constant_text!(done_title, "Done");

/// The first local, a string, as a label shows it.
unsafe extern "C" fn first_local(_: *const Context, frame: *const Slot, out: *mut Slot) -> bool {
    // SAFETY: the frame's first local is a string, and `out` is writable.
    unsafe { out.write(kept(*frame, &STRING)) };
    true
}

unsafe extern "C" fn hi(_: *const Context, frame: *const Slot, out: *mut Slot) -> bool {
    // SAFETY: the frame's first local is a string, the builder is used once, and `out` is
    // writable.
    unsafe {
        let text = nf_text_new();
        nf_text_push_str(text, "Hi ".as_ptr(), "Hi ".len());
        nf_text_push(text, *frame, &STRING);
        out.write(nf_text_finish(text));
    }
    true
}

unsafe extern "C" fn many_or_few(context: *const Context, _: *const Slot, sink: *mut Sink) -> bool {
    // SAFETY: the context and sink are live.
    unsafe {
        let few = int(nf_state(context, COUNT)) <= 1;
        nf_show_same(sink, u32::from(few));
    }
    true
}

unsafe extern "C" fn each_name(context: *const Context, _: *const Slot, sink: *mut Sink) -> bool {
    // SAFETY: the context and sink are live, `names` is a list of strings, and each item
    // frame's first local is the loop's string.
    unsafe {
        let names = nf_state(context, NAMES);
        for index in 0..nf_list_len(names) {
            let mut name = 0;
            if !nf_list_get(context, names, index, 0, &raw mut name) {
                return false;
            }
            let locals = nf_show_item(sink, 0);
            nf_store(locals, kept(name, &STRING), &STRING);
        }
    }
    true
}

unsafe extern "C" fn greeting(context: *const Context, _: *const Slot, sink: *mut Sink) -> bool {
    // SAFETY: the context and sink are live, and the view's first local is its string
    // parameter.
    unsafe {
        let locals = nf_show_view(sink, 0);
        nf_store(locals, kept(nf_state(context, DRAFT), &STRING), &STRING);
    }
    true
}

unsafe extern "C" fn read_draft(context: *const Context, _: *const Slot, out: *mut Slot) -> bool {
    // SAFETY: the context is live, and `out` is writable.
    unsafe { out.write(kept(nf_state(context, DRAFT), &STRING)) };
    true
}

unsafe extern "C" fn write_draft(context: *const Context, _: *const Slot, value: Slot) -> bool {
    // SAFETY: the context is live, and `value` is an owned string.
    unsafe { nf_store(nf_state_mut(context, DRAFT), value, &STRING) };
    true
}

unsafe extern "C" fn read_done(context: *const Context, _: *const Slot, out: *mut Slot) -> bool {
    // SAFETY: the context is live, and `out` is writable.
    unsafe { out.write(nf_state(context, DONE)) };
    true
}

unsafe extern "C" fn write_done(context: *const Context, _: *const Slot, value: Slot) -> bool {
    // SAFETY: the context is live.
    unsafe { nf_state_mut(context, DONE).write(value) };
    true
}

unsafe extern "C" fn add_one(context: *const Context, _: *mut Slot) -> bool {
    // SAFETY: the context is live, and `count` is an int.
    unsafe {
        let count = nf_state_mut(context, COUNT);
        *count = int_slot(int(*count) + 1);
    }
    true
}

unsafe extern "C" fn push_b(context: *const Context, _: *mut Slot) -> bool {
    // SAFETY: the context is live, and `names` is a list of strings.
    unsafe { nf_list_push(nf_state_mut(context, NAMES), literal("b")) };
    true
}

unsafe extern "C" fn divide_by_zero(context: *const Context, _: *mut Slot) -> bool {
    // SAFETY: the context is live.
    unsafe { nf_fail_divide_by_zero(context, DIVISION) };
    false
}

const fn label(text: ReadFn, deps: &'static [u32]) -> TemplateDesc {
    TemplateDesc {
        text: Some(text),
        deps: Slice::new(deps),
        ..TemplateDesc::empty(TemplateKind::Label)
    }
}

const fn button(title: ReadFn, action: ActionFn) -> TemplateDesc {
    TemplateDesc {
        text: Some(title),
        action: Some(action),
        button: ButtonStyle::Normal,
        ..TemplateDesc::empty(TemplateKind::Button)
    }
}

const fn region(
    expand: ExpandFn,
    lists: &'static [Slice<TemplateDesc>],
    deps: &'static [u32],
) -> TemplateDesc {
    TemplateDesc {
        region: RegionDesc {
            expand: Some(expand),
            lists: Slice::new(lists),
        },
        deps: Slice::new(deps),
        ..TemplateDesc::empty(TemplateKind::Region)
    }
}

static WINDOW: [TemplateDesc; 9] = [
    label(clicked, &[COUNT]),
    region(
        many_or_few,
        &[
            Slice::new(&[label(many, &[])]),
            Slice::new(&[label(few, &[])]),
        ],
        &[COUNT],
    ),
    region(
        each_name,
        &[Slice::new(&[label(first_local, &[])])],
        &[NAMES],
    ),
    region(greeting, &[], &[DRAFT]),
    TemplateDesc {
        binding: BindingDesc {
            read: Some(read_draft),
            write: Some(write_draft),
        },
        placeholder: Str::new("Name"),
        deps: Slice::new(&[DRAFT]),
        ..TemplateDesc::empty(TemplateKind::TextField)
    },
    TemplateDesc {
        binding: BindingDesc {
            read: Some(read_done),
            write: Some(write_done),
        },
        text: Some(done_title),
        deps: Slice::new(&[DONE]),
        ..TemplateDesc::empty(TemplateKind::Checkbox)
    },
    button(plus, add_one),
    button(add, push_b),
    button(break_title, divide_by_zero),
];

static PROGRAM: ProgramDesc = ProgramDesc {
    abi_version: ABI_VERSION,
    states: Slice::new(&[
        StateDesc {
            ty: &INT,
            init: zero,
        },
        StateDesc {
            ty: &LIST_OF_STRINGS,
            init: just_a,
        },
        StateDesc {
            ty: &STRING,
            init: empty_string,
        },
        StateDesc {
            ty: &BOOL,
            init: no,
        },
    ]),
    windows: Slice::new(&[WindowDesc {
        title: Str::new("Test"),
        width: 320.0,
        height: 240.0,
        backdrop: nifty_runtime::Backdrop::Standard,
        style: nifty_runtime::StackStyle::new(nifty_runtime::Axis::Vertical),
        children: Slice::new(&WINDOW),
        frame: FrameDesc {
            locals: Slice::new(&[&STRING]),
        },
    }]),
    views: Slice::new(&[ViewDesc {
        body: Slice::new(&[label(hi, &[])]),
        frame: FrameDesc {
            locals: Slice::new(&[&STRING]),
        },
    }]),
    functions: Slice::EMPTY,
    spans: Slice::new(&[SpanDesc {
        file: Str::new("main.nf"),
        line: 19,
        column: 29,
    }]),
    menus: Slice::new(&[MenuDesc {
        title: Str::new("Count"),
        entries: Slice::new(&[
            MenuEntryDesc {
                kind: MenuEntryKind::Item,
                title: Str::new("Add One"),
                shortcut: ShortcutDesc {
                    present: true,
                    shortcut: Shortcut::command(Key::Equals),
                },
                action: Some(add_one),
                entries: Slice::EMPTY,
            },
            MenuEntryDesc {
                kind: MenuEntryKind::Separator,
                title: Str::EMPTY,
                shortcut: NO_SHORTCUT,
                action: None,
                entries: Slice::EMPTY,
            },
        ]),
    }]),
    menu_frame: FrameDesc {
        locals: Slice::EMPTY,
    },
};

/// An item's shortcut field when it has none.
const NO_SHORTCUT: ShortcutDesc = ShortcutDesc {
    present: false,
    shortcut: Shortcut::command(Key::A),
};

const EMPTY: ProgramDesc = ProgramDesc {
    abi_version: ABI_VERSION,
    states: Slice::EMPTY,
    windows: Slice::EMPTY,
    views: Slice::EMPTY,
    functions: Slice::EMPTY,
    spans: Slice::EMPTY,
    menus: Slice::EMPTY,
    menu_frame: FrameDesc {
        locals: Slice::EMPTY,
    },
};

/// A context for a program with no state, for testing runtime calls.
pub(crate) fn empty_context() -> Context {
    static PROGRAM: ProgramDesc = EMPTY;
    Context::new(&PROGRAM).expect("an empty program starts")
}

struct Running {
    engine: Rc<Engine<Compiled, FakeBackend>>,
    fake: FakeBackend,
    errors: Rc<RefCell<Vec<String>>>,
}

fn start_test(program: &'static ProgramDesc) -> Result<Running, AppError> {
    let fake = FakeBackend::default();
    let errors = Rc::new(RefCell::new(Vec::new()));
    let on_error = Rc::new({
        let errors = Rc::clone(&errors);
        move |error: AppError| errors.borrow_mut().push(error.to_string())
    });
    let engine = prepare(program)?.show(Ui::new(fake.clone()), on_error)?;
    Ok(Running {
        engine,
        fake,
        errors,
    })
}

impl Running {
    fn labels(&self) -> Vec<String> {
        self.fake.visible_texts(FakeKind::Label)
    }

    fn click(&self, title: &str) {
        let button = self
            .fake
            .find(FakeKind::Button)
            .into_iter()
            .find(|button| self.fake.text(*button) == title)
            .unwrap_or_else(|| panic!("no button titled {title:?}"));
        self.fake.click(button);
    }
}

#[test]
fn runs_a_compiled_program() {
    let app = start_test(&PROGRAM).expect("the program starts");
    assert_eq!(app.fake.window_title(0), "Test");
    assert_eq!(app.labels(), ["Clicked 0 times", "few", "a", "Hi "]);

    app.click("+");
    app.click("+");
    app.click("Add");
    assert_eq!(app.labels(), ["Clicked 2 times", "many", "a", "b", "Hi "]);

    let field = app.fake.find(FakeKind::TextField)[0];
    assert_eq!(app.fake.placeholder(field), "Name");
    app.fake.type_text(field, "Ada");
    assert_eq!(app.labels().last().map(String::as_str), Some("Hi Ada"));

    app.fake.choose_menu_item(&["Count", "Add One"]);
    assert_eq!(app.labels()[0], "Clicked 3 times");

    let checkbox = app.fake.find(FakeKind::Checkbox)[0];
    app.fake.toggle(checkbox);
    // SAFETY: the context is live, and `done` is a bool.
    let done = unsafe { nf_state(app.engine.logic().context(), DONE) };
    assert_eq!(done, 1);
    assert!(app.errors.borrow().is_empty());
}

#[test]
fn failures_point_at_their_code() {
    let app = start_test(&PROGRAM).expect("the program starts");
    app.click("Break");
    assert_eq!(
        *app.errors.borrow(),
        ["main.nf:19:29: can't divide by zero"]
    );
}

#[test]
fn programs_built_for_another_runtime_are_refused() {
    static OLD: ProgramDesc = ProgramDesc {
        abi_version: ABI_VERSION + 1,
        ..EMPTY
    };
    let error = start_test(&OLD).err().expect("an old program is refused");
    assert!(
        error.message.contains("built for runtime version"),
        "{error}"
    );
}

#[test]
fn templates_missing_what_their_kind_needs_are_refused() {
    static BROKEN: ProgramDesc = ProgramDesc {
        windows: Slice::new(&[WindowDesc {
            title: Str::new("Broken"),
            width: 100.0,
            height: 100.0,
            backdrop: nifty_runtime::Backdrop::Standard,
            style: nifty_runtime::StackStyle::new(nifty_runtime::Axis::Vertical),
            children: Slice::new(&[TemplateDesc::empty(TemplateKind::Label)]),
            frame: FrameDesc {
                locals: Slice::EMPTY,
            },
        }]),
        ..EMPTY
    };
    let error = start_test(&BROKEN)
        .err()
        .expect("a label without text is refused");
    assert!(
        error.message.contains("a label's text is missing"),
        "{error}"
    );
}

/// Stands for:
///
/// ```nifty
/// state url = ""
/// state folder = ""
/// state percent = 0
/// state status = ""
///
/// window("Downloader") {
///   text_field(url)
///   text_field(folder)
///   button("Get") { download(url, to: folder, progress: $percent, status: $status) }
///   label("{percent}% {status}")
/// }
/// ```
mod downloader {
    use std::fs;

    use nifty_download::testing::{scratch_folder, serve_once};

    use super::*;
    use crate::download::nf_download;
    use crate::place::{PathStep, PlaceDesc};

    const URL: u32 = 0;
    const FOLDER: u32 = 1;
    const PERCENT: u32 = 2;
    const STATUS: u32 = 3;

    macro_rules! string_state {
        ($read:ident, $write:ident, $state:expr) => {
            unsafe extern "C" fn $read(
                context: *const Context,
                _: *const Slot,
                out: *mut Slot,
            ) -> bool {
                // SAFETY: the context is live, and `out` is writable.
                unsafe { out.write(kept(nf_state(context, $state), &STRING)) };
                true
            }

            unsafe extern "C" fn $write(
                context: *const Context,
                _: *const Slot,
                value: Slot,
            ) -> bool {
                // SAFETY: the context is live, and `value` is an owned string.
                unsafe { nf_store(nf_state_mut(context, $state), value, &STRING) };
                true
            }
        };
    }

    string_state!(read_url, write_url, URL);
    string_state!(read_folder, write_folder, FOLDER);
    constant_text!(get, "Get");

    unsafe extern "C" fn start_download(context: *const Context, _: *mut Slot) -> bool {
        let state = |state| PlaceDesc {
            state,
            steps: std::ptr::null::<PathStep>(),
            len: 0,
        };
        // SAFETY: the context is live, both places are whole states, and the url and
        // folder are strings.
        unsafe {
            nf_download(
                context,
                nf_state(context, URL),
                nf_state(context, FOLDER),
                &state(PERCENT),
                &state(STATUS),
                0,
            )
        }
    }

    unsafe extern "C" fn progress(context: *const Context, _: *const Slot, out: *mut Slot) -> bool {
        // SAFETY: the context is live, the builder is used once, and `out` is writable.
        unsafe {
            let text = nf_text_new();
            nf_text_push(text, nf_state(context, PERCENT), &INT);
            nf_text_push_str(text, "% ".as_ptr(), "% ".len());
            nf_text_push(text, nf_state(context, STATUS), &STRING);
            out.write(nf_text_finish(text));
        }
        true
    }

    const fn text_field(read: ReadFn, write: WriteFn, state: &'static [u32]) -> TemplateDesc {
        TemplateDesc {
            binding: BindingDesc {
                read: Some(read),
                write: Some(write),
            },
            deps: Slice::new(state),
            ..TemplateDesc::empty(TemplateKind::TextField)
        }
    }

    static PROGRAM: ProgramDesc = ProgramDesc {
        states: Slice::new(&[
            StateDesc {
                ty: &STRING,
                init: empty_string,
            },
            StateDesc {
                ty: &STRING,
                init: empty_string,
            },
            StateDesc {
                ty: &INT,
                init: zero,
            },
            StateDesc {
                ty: &STRING,
                init: empty_string,
            },
        ]),
        windows: Slice::new(&[WindowDesc {
            title: Str::new("Downloader"),
            width: 320.0,
            height: 240.0,
            backdrop: nifty_runtime::Backdrop::Standard,
            style: nifty_runtime::StackStyle::new(nifty_runtime::Axis::Vertical),
            children: Slice::new(&[
                text_field(read_url, write_url, &[URL]),
                text_field(read_folder, write_folder, &[FOLDER]),
                button(get, start_download),
                label(progress, &[PERCENT, STATUS]),
            ]),
            frame: FrameDesc {
                locals: Slice::EMPTY,
            },
        }]),
        ..EMPTY
    };

    #[test]
    fn saves_the_file_and_reports_when_done() {
        let folder = scratch_folder("rt-download-saves");
        let url = format!("{}/files/hello.txt", serve_once("200 OK", "hello, nifty"));
        let app = start_test(&PROGRAM).expect("the program starts");
        let [url_field, folder_field] = app.fake.find(FakeKind::TextField)[..] else {
            panic!("the downloader has an address field and a folder field");
        };
        app.fake.type_text(url_field, &url);
        app.fake
            .type_text(folder_field, &folder.display().to_string());

        app.click("Get");
        assert_eq!(
            app.labels(),
            ["0% Downloading…"],
            "the click shows it has begun"
        );

        app.engine.logic().wait_for_downloads();
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
}

/// Stands for:
///
/// ```nifty
/// state photo = ""
/// state paths: [string] = []
///
/// window("Picker") {
///   button("Open") { pick_file($photo, title: "Choose", from: "~") }
///   button("Attach") { pick_files($paths) }
///   label(photo)
///   label("{paths.len()} attached")
/// }
/// ```
mod picker {
    use nifty_runtime::{DialogKind, Picked};

    use super::*;
    use crate::abi::nf_release;
    use crate::dialog::nf_pick;
    use crate::place::{PathStep, PlaceDesc};

    const PHOTO: u32 = 0;
    const PATHS: u32 = 1;

    constant_text!(open, "Open");
    constant_text!(attach, "Attach");

    unsafe extern "C" fn no_paths(_: *const Context, out: *mut Slot) -> bool {
        // SAFETY: an empty list reads no items, and `out` is writable.
        unsafe { out.write(nf_list_new(&LIST_OF_STRINGS, std::ptr::null(), 0)) };
        true
    }

    /// Calls `nf_pick` as compiled code does, with owned temporaries it releases after.
    ///
    /// # Safety
    /// `context` is live.
    unsafe fn pick(
        context: *const Context,
        kind: DialogKind,
        state: u32,
        title: &str,
        folder: &str,
    ) -> bool {
        let place = PlaceDesc {
            state,
            steps: std::ptr::null::<PathStep>(),
            len: 0,
        };
        // SAFETY: every slot is an owned value of the type `nf_pick` borrows, released
        // once it returns, and the place is a whole state.
        unsafe {
            let strings = [literal(title), literal(folder), literal("")];
            let types = nf_list_new(&LIST_OF_STRINGS, std::ptr::null(), 0);
            let asked = nf_pick(
                context, kind, &place, strings[0], types, strings[1], strings[2], 0,
            );
            nf_release(types, &LIST_OF_STRINGS);
            for string in strings {
                nf_release(string, &STRING);
            }
            asked
        }
    }

    unsafe extern "C" fn pick_photo(context: *const Context, _: *mut Slot) -> bool {
        // SAFETY: the context is live.
        unsafe { pick(context, DialogKind::File, PHOTO, "Choose", "~") }
    }

    unsafe extern "C" fn pick_paths(context: *const Context, _: *mut Slot) -> bool {
        // SAFETY: the context is live.
        unsafe { pick(context, DialogKind::Files, PATHS, "", "") }
    }

    unsafe extern "C" fn photo(context: *const Context, _: *const Slot, out: *mut Slot) -> bool {
        // SAFETY: the context is live, and `out` is writable.
        unsafe { out.write(kept(nf_state(context, PHOTO), &STRING)) };
        true
    }

    unsafe extern "C" fn attached(context: *const Context, _: *const Slot, out: *mut Slot) -> bool {
        const SUFFIX: &str = " attached";
        // SAFETY: the context is live, the builder is used once, and `out` is writable.
        unsafe {
            let text = nf_text_new();
            let count = int_slot(nf_list_len(nf_state(context, PATHS)));
            nf_text_push(text, count, &INT);
            nf_text_push_str(text, SUFFIX.as_ptr(), SUFFIX.len());
            out.write(nf_text_finish(text));
        }
        true
    }

    static PROGRAM: ProgramDesc = ProgramDesc {
        states: Slice::new(&[
            StateDesc {
                ty: &STRING,
                init: empty_string,
            },
            StateDesc {
                ty: &LIST_OF_STRINGS,
                init: no_paths,
            },
        ]),
        windows: Slice::new(&[WindowDesc {
            title: Str::new("Picker"),
            width: 320.0,
            height: 240.0,
            backdrop: nifty_runtime::Backdrop::Standard,
            style: nifty_runtime::StackStyle::new(nifty_runtime::Axis::Vertical),
            children: Slice::new(&[
                button(open, pick_photo),
                button(attach, pick_paths),
                label(photo, &[PHOTO]),
                label(attached, &[PATHS]),
            ]),
            frame: FrameDesc {
                locals: Slice::EMPTY,
            },
        }]),
        ..EMPTY
    };

    #[test]
    fn shows_the_dialog_once_the_click_has_finished_and_writes_the_answer() {
        let app = start_test(&PROGRAM).expect("the program starts");

        app.click("Open");
        assert_eq!(app.fake.dialogs(), []);
        app.fake.run_pending_wakes();
        let [dialog] = &app.fake.dialogs()[..] else {
            panic!("the click shows one dialog");
        };
        assert_eq!(dialog.kind, DialogKind::File);
        assert_eq!(dialog.title, "Choose");
        assert_eq!(dialog.folder, std::env::home_dir());

        app.fake
            .close_dialog(0, Picked::Chosen(vec!["/photos/cat.png".to_owned()]));
        app.fake.run_pending_wakes();
        assert_eq!(app.labels(), ["/photos/cat.png", "0 attached"]);
        assert!(app.errors.borrow().is_empty());
    }

    #[test]
    fn picking_files_replaces_the_list_and_cancelling_keeps_it() {
        let app = start_test(&PROGRAM).expect("the program starts");
        app.click("Attach");
        app.fake.run_pending_wakes();
        app.fake.close_dialog(
            0,
            Picked::Chosen(vec!["/a.txt".to_owned(), "/b.txt".to_owned()]),
        );
        app.fake.run_pending_wakes();

        app.click("Attach");
        app.fake.run_pending_wakes();
        app.fake.close_dialog(1, Picked::Cancelled);
        app.fake.run_pending_wakes();

        assert_eq!(app.labels(), ["", "2 attached"]);
    }

    #[test]
    fn a_failed_dialog_reports_an_error() {
        let app = start_test(&PROGRAM).expect("the program starts");
        app.click("Open");
        app.fake.run_pending_wakes();
        app.fake
            .close_dialog(0, Picked::Failed("no display".to_owned()));
        app.fake.run_pending_wakes();

        assert_eq!(*app.errors.borrow(), ["the file dialog failed: no display"]);
    }
}
