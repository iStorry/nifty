use nifty_runtime::{
    ButtonStyle, Icon, ImageSize, Key, LabelStyle, StackStyle, Surface, TextAlign, TextColor,
    TextStyle, ToggleStyle,
};

use nifty_syntax::{FileId, Span};

use crate::elements::{
    BoundType, ELEMENTS, Element, ElementKind, Options, Param, ParamKind, Place, TrailingBlock,
    WINDOW,
};
use crate::functions::{DOWNLOAD, FUNCTIONS, Function, FunctionKind};
use crate::hir::{
    DialogKind, ExprKind, MenuEntry, Node, Program, Shortcut, StateId, Stmt, Type, WindowAction,
    WindowId,
};
use crate::methods::{METHODS, MethodKind};
use crate::{check, compile};

fn parse(source: &str) -> nifty_syntax::Parsed {
    let parsed = nifty_syntax::parse(source, FileId::default());
    assert_eq!(parsed.diagnostics, [], "the test source must parse");
    parsed
}

fn program(source: &str) -> Program {
    let checked = check(&[&parse(source).file]);
    assert_eq!(checked.diagnostics, [], "the test source must check");
    checked.program
}

fn errors(source: &str) -> Vec<String> {
    check(&[&parse(source).file])
        .diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect()
}

/// Wraps UI in a window, and declarations before it.
fn app(declarations: &str, ui: &str) -> String {
    format!("{declarations}\nwindow(\"Test\") {{\n{ui}\n}}")
}

fn content(program: &Program) -> &[Node] {
    &program.windows[0].children
}

fn deps(node: &Node) -> Vec<StateId> {
    match node {
        Node::Label { text, .. }
        | Node::Progress {
            value: Some(text), ..
        } => text.deps.iter().collect(),
        Node::If { deps, .. } | Node::For { deps, .. } | Node::View { deps, .. } => {
            deps.iter().collect()
        }
        other => panic!("{other:?} has no dependencies"),
    }
}

#[test]
fn checks_the_counter_example() {
    let program = program(include_str!("../../../fixtures/counter.nf"));
    assert_eq!(program.states[0].ty, Type::Int);
    let window = &program.windows[0];
    assert_eq!(window.title, "Counter");
    assert_eq!((window.size.width, window.size.height), (320.0, 200.0));
    assert_eq!(deps(&content(&program)[0]), [StateId(0)]);
}

#[test]
fn checks_the_todos_example() {
    let program = program(include_str!("../../../fixtures/todos.nf"));
    let nodes = content(&program);
    assert_eq!(
        deps(&nodes[0]),
        [StateId(0)],
        "Header reads todos through remaining()"
    );
    assert!(matches!(nodes[3], Node::For { .. }));
    assert_eq!(deps(&nodes[3]), [StateId(0)]);
}

#[test]
fn checks_the_downloader_example() {
    program(include_str!("../../../fixtures/downloader.nf"));
}

#[test]
fn checks_the_sidebar_example() {
    program(include_str!("../../../fixtures/sidebar.nf"));
}

#[test]
fn checks_the_files_example() {
    program(include_str!("../../../fixtures/files.nf"));
}

#[test]
fn the_gallery_example_shows_every_element_and_option() {
    let source = include_str!("../../../fixtures/gallery.nf");
    program(source);
    for element in ELEMENTS {
        assert!(
            source.contains(&format!("{}(", element.name)),
            "the gallery is missing `{}`",
            element.name
        );
        for param in element.params {
            if let ParamKind::Option(options) = param.kind {
                for name in options.names() {
                    assert!(
                        source.contains(&format!("{}: .{name}", param.name)),
                        "the gallery is missing `{}: .{name}`",
                        param.name
                    );
                }
            }
        }
    }
}

#[test]
fn every_icon_but_none_can_be_written() {
    let names = Options::Icon.names();
    assert_eq!(names.len(), Icon::ALL.len() - 1, "{names:?}");
    let program = program(&app(
        "state place = \"a\"",
        "nav_item(\"A\", value: \"a\", selection: $place, icon: .star_fill, badge: \"2\")\nbutton(\"Go\", style: .icon, icon: .trash) { place = \"b\" }",
    ));
    assert!(matches!(&content(&program)[0], Node::NavItem(item) if item.icon == Icon::StarFill));
    assert!(matches!(
        content(&program)[1],
        Node::Button {
            icon: Icon::Trash,
            style: ButtonStyle::Icon,
            ..
        }
    ));
}

#[test]
fn nav_items_need_a_value_and_a_selection() {
    let errors = errors(&app("state s = \"\"", "nav_item(\"A\")"));
    assert!(errors.iter().any(|e| e.contains("`value:`")), "{errors:?}");
    assert!(errors.iter().any(|e| e.contains("selection")), "{errors:?}");
}

#[test]
fn progress_shows_an_int_that_may_change() {
    let program = program(&app(
        "state percent = 0",
        "progress(value: percent)\nprogress()",
    ));
    let [first, second] = content(&program) else {
        panic!("the window shows two progress bars");
    };
    assert_eq!(deps(first), [StateId(0)]);
    assert!(matches!(second, Node::Progress { value: None }));
    assert_eq!(
        errors(&app("state done = \"half\"", "progress(value: done)")),
        ["expected `int`, found `string`"]
    );
}

const CONTROL_STATE: &str = "state volume = 5\nstate quality = \"High\"\nstate wifi = true\nstate choices: [string] = [\"Low\", \"High\"]";

#[test]
fn switches_sliders_and_pickers_edit_their_binding() {
    let program = program(&app(
        CONTROL_STATE,
        "switch(on: $wifi, title: \"Wi-Fi\")\nslider($volume, min: -10, max: 10)\nslider($volume)\npicker($quality, options: choices)",
    ));
    let [switch, ranged, default, picker] = content(&program) else {
        panic!("the window shows four controls");
    };
    assert!(matches!(
        switch,
        Node::Checkbox {
            style: ToggleStyle::Switch,
            ..
        }
    ));
    assert!(matches!(
        ranged,
        Node::Slider {
            min: -10,
            max: 10,
            ..
        }
    ));
    assert!(matches!(
        default,
        Node::Slider {
            min: 0,
            max: 100,
            ..
        }
    ));
    let Node::Picker { deps, .. } = picker else {
        panic!("the last control is a picker");
    };
    assert_eq!(deps.iter().collect::<Vec<_>>(), [StateId(1), StateId(3)]);
}

#[test]
fn controls_report_bad_ranges_options_and_bindings() {
    assert_eq!(
        errors(&app(
            CONTROL_STATE,
            "slider($volume, min: 5, max: 1)\nslider($volume, min: volume)\npicker($quality)\nswitch(checked: $wifi)",
        )),
        [
            "`slider` needs `min` no higher than `max`, but 5 is above 1",
            "this must be a whole number such as `10`",
            "`picker` needs `options:`, the strings to pick from, such as `options: [\"Low\", \"High\"]`",
            "`switch` needs a two-way binding to a `bool`, such as `on: $name`",
        ]
    );
}

#[test]
fn buttons_show_a_selection_that_may_change() {
    let program = program(&app(
        "state filter = \"all\"",
        "button(\"All\", style: .plain, selected: filter == \"all\") { filter = \"all\" }",
    ));
    let [
        Node::Button {
            selected: Some(selected),
            ..
        },
    ] = content(&program)
    else {
        panic!("the window shows one selectable button");
    };
    assert_eq!(selected.deps.iter().collect::<Vec<_>>(), [StateId(0)]);
    assert_eq!(
        errors(&app(
            "state filter = \"all\"",
            "button(\"All\", selected: filter) { filter = \"all\" }\nbutton(\"Go\", style: .primary, selected: true) { filter = \"go\" }",
        )),
        [
            "expected `bool`, found `string`",
            "a `.primary` button is the window's default action, so it can't be `selected`; pick another style",
        ]
    );
}

#[test]
fn text_fields_can_be_several_lines_tall() {
    let program = program(&app(
        "state notes = \"\"",
        "text_field($notes, lines: 5)\ntext_field($notes, placeholder: \"Name\")",
    ));
    assert!(matches!(
        content(&program),
        [
            Node::TextField { lines: 5, .. },
            Node::TextField { lines: 1, .. }
        ]
    ));
    assert_eq!(
        errors(&app(
            "state notes = \"\"",
            "text_field($notes, lines: 0)\ntext_field($notes, placeholder: \"Notes\", lines: 3)",
        )),
        [
            "this must be between 1 and 100",
            "`text_field` shows a placeholder only on one line, with no `lines:`",
        ]
    );
}

#[test]
fn images_take_a_full_path_and_a_size() {
    let program = program(&app(
        "state cover = \"\"",
        "image(cover, width: 64)\nimage(\"~/Pictures/a.png\")\nimage(\"C:\\\\Art\\\\b.png\", height: 10)",
    ));
    let [Node::Image { path, size }, ..] = content(&program) else {
        panic!("the window shows images");
    };
    assert_eq!(path.deps.iter().collect::<Vec<_>>(), [StateId(0)]);
    assert_eq!(
        *size,
        ImageSize {
            width: 64.0,
            height: ImageSize::FIT
        }
    );
    assert_eq!(
        errors(&app("", "image(\"logo.png\")\nimage(\"~me/logo.png\")")),
        [
            "`image` needs a full path, or one starting with `~`; images that come with an app aren't supported yet",
            "`image` needs a full path, or one starting with `~`; images that come with an app aren't supported yet",
        ]
    );
}

#[test]
fn a_one_line_text_field_can_run_an_action_on_return() {
    let program = program(&app(
        "state query = \"\"\nstate searched = \"\"",
        "text_field($query) { searched = query }",
    ));
    assert!(matches!(
        content(&program),
        [Node::TextField {
            action: Some(_),
            ..
        }]
    ));
    assert_eq!(
        errors(&app(
            "state notes = \"\"",
            "text_field($notes, lines: 3) { notes = \"\" }",
        )),
        [
            "`text_field` takes Return as a new line when it has `lines:`, so it can't take a `{ … }` block to run on Return"
        ]
    );
}

const MENUS: &str = "menu(\"File\") {\n  menu_item(\"New\", shortcut: \"n\") { count += 1 }\n  divider()\n  menu(\"Sort\") {\n    menu_item(\"By Name\", shortcut: \"shift+alt+1\") {\n      let step = 2\n      count += step\n    }\n  }\n}\nmenu(\"Help\") {\n  menu_item(\"About\") { count = 0 }\n}";

#[test]
fn menus_build_the_menu_bar_in_order() {
    let program = program(&format!("{}\n{MENUS}", app("state count = 0", "")));
    let titles: Vec<&str> = program
        .menu_bar
        .menus
        .iter()
        .map(|menu| menu.title.as_str())
        .collect();
    assert_eq!(titles, ["File", "Help"]);
    let [new, MenuEntry::Separator, MenuEntry::Submenu(sort)] =
        &program.menu_bar.menus[0].entries[..]
    else {
        panic!("File has an item, a separator and a submenu");
    };
    assert!(matches!(
        new,
        MenuEntry::Item {
            shortcut: Some(Shortcut {
                key: Key::N,
                shift: false,
                alt: false
            }),
            ..
        }
    ));
    assert_eq!(sort.title, "Sort");
    assert_eq!(program.menu_bar.locals, [Type::Int]);
}

#[test]
fn menus_report_misplaced_elements_and_shortcuts() {
    let errors = errors(&format!(
        "{}\nmenu(\"File\") {{\n  label(\"x\")\n  menu_item(\"Copy\", shortcut: \"c\") {{ count = 1 }}\n  menu_item(\"A\", shortcut: \"ctrl+a\") {{ count = 1 }}\n  menu_item(\"B\", shortcut: \"b\") {{ count = 1 }}\n  menu_item(\"C\", shortcut: \"b\") {{ count = 2 }}\n}}\nmenu_item(\"Stray\") {{ count = 3 }}",
        app(
            "state count = 0",
            "menu_item(\"In a window\") { count = 4 }"
        )
    ));
    assert_eq!(
        errors,
        [
            "`menu_item` goes in the app's menu bar: in a top-level `menu(…)`",
            "only `menu_item`, `menu` and `divider` can go in a menu",
            "`c` is the system's shortcut for Copy; choose another",
            "`ctrl` isn't a modifier; a shortcut always uses Cmd on macOS and Ctrl on Windows, and may add `shift` and `alt`",
            "another menu item already uses `b`",
            "only declarations, `window(…)` and `menu(…)` can appear at the top level",
        ]
    );
}

#[test]
fn drop_targets_bind_a_list_and_may_run_an_action() {
    let program = program(&app(
        "state dropped: [string] = []\nstate count = 0",
        "drop_target($dropped) { count += 1 }\ncontext_menu() { divider() }",
    ));
    assert!(matches!(
        content(&program),
        [
            Node::DropTarget {
                action: Some(_),
                ..
            },
            Node::ContextMenu { .. }
        ]
    ));
    assert_eq!(
        errors(&app(
            "state dropped: [string] = []\nstate name = \"\"",
            "drop_target($dropped)\ndrop_target($name)",
        )),
        [
            "a stack has one `drop_target`",
            "expected `[string]`, found `string`",
        ]
    );
}

#[test]
fn named_windows_open_and_close_from_code() {
    let program = program(
        "window main(\"Main\") {\n  button(\"Settings\") { open_window(settings) }\n}\nwindow settings(\"Settings\", open: false) {\n  button(\"Done\") { close_window(settings) }\n}",
    );
    let [main, settings] = &program.windows[..] else {
        panic!("the app has two windows");
    };
    assert!(main.starts_open);
    assert!(!settings.starts_open);
    let actions = [main, settings].map(|window| match &window.children[..] {
        [Node::Button { action, .. }] => match &action[..] {
            [Stmt::Expr(call)] => match call.kind {
                ExprKind::Window(action, window) => (action, window),
                _ => panic!("the action asks for a window"),
            },
            _ => panic!("the action is one call"),
        },
        _ => panic!("each window shows a button"),
    });
    assert_eq!(
        actions,
        [
            (WindowAction::Open, WindowId(1)),
            (WindowAction::Close, WindowId(1))
        ]
    );
}

#[test]
fn window_names_are_only_for_opening_and_closing() {
    let errors = errors(
        "state count = 0\nfn shown() -> string {\n  open_window(other)\n  return \"\"\n}\nwindow main(\"Main\") {\n  label(\"{other}\")\n  label(shown())\n  button(\"A\") { open_window(count) }\n  button(\"B\") { close_window() }\n}\nwindow other(\"Other\") {\n}\nwindow(\"Hidden\", open: false) {\n}\nmenu file(\"File\") {\n  divider()\n}",
    );
    assert_eq!(
        errors,
        [
            "`other` is a window; open it with `open_window(other)`",
            "showing the UI can't change state; do that in a button's action",
            "`count` isn't the name of a window; name one with `window count(\"Title\") { … }`",
            "`close_window` needs a window's name, such as `close_window(settings)`",
            "a window that starts closed needs a name to open it by, such as `window settings(…)`",
            "only a `window` takes a name, to open and close it by",
        ]
    );
}

#[test]
fn an_app_needs_a_window_that_starts_open() {
    assert_eq!(
        errors("window a(\"A\", open: false) {\n}"),
        ["a Nifty app needs a window that opens when it starts; leave `open: false` off one"]
    );
}

#[test]
fn context_menus_hold_items_that_run_in_the_ui() {
    let program = program(&app(
        "state names: [string] = []\nstate opened = \"\"",
        "for name in names {\n  row() {\n    context_menu() {\n      menu_item(\"Open\") { opened = name }\n      divider()\n      menu(\"More\") { menu_item(\"Clear\") { opened = \"\" } }\n    }\n    label(name)\n  }\n}",
    ));
    let [Node::For { body, .. }] = content(&program) else {
        panic!("the window shows a list");
    };
    let [Node::Stack { children, .. }] = &body[..] else {
        panic!("each item is a row");
    };
    let [Node::ContextMenu { entries }, Node::Label { .. }] = &children[..] else {
        panic!("each row has a context menu and a label");
    };
    assert!(matches!(
        &entries[..],
        [
            MenuEntry::Item { .. },
            MenuEntry::Separator,
            MenuEntry::Submenu(_)
        ]
    ));
}

#[test]
fn context_menus_report_shortcuts_and_repeats() {
    let errors = errors(&app(
        "state names: [string] = []\nstate count = 0",
        "context_menu() { menu_item(\"A\", shortcut: \"n\") { count = 1 } }\ncontext_menu() { divider() }\nfor name in names {\n  context_menu() { divider() }\n}\ncontext_menu()",
    ));
    assert_eq!(
        errors,
        [
            "a context menu's items take no shortcut; give it to an item of a top-level `menu(…)`",
            "a stack has one `context_menu`",
            "a `context_menu` here would cover the stack around the `for` once per item; put it in a `row` or `column` inside the loop",
            "`context_menu` needs a `{ … }` block with its items",
            "a stack has one `context_menu`",
        ]
    );
}

#[test]
fn stacks_take_a_surface_and_sizes() {
    let program = program(&app(
        "",
        "row() {\n  column(surface: .card, width: 120, max_width: 300, grow: true) { label(\"a\") }\n  column() { label(\"b\") }\n}",
    ));
    // The window's one top-level `row` is its own stack, so its columns are the content.
    let styles: Vec<StackStyle> = content(&program)
        .iter()
        .map(|child| match child {
            Node::Stack { style, .. } => *style,
            other => panic!("expected a column, found {other:?}"),
        })
        .collect();
    assert_eq!(styles[0].surface, Surface::Card);
    assert_eq!(styles[0].fixed_width(), Some(120.0));
    assert_eq!(styles[0].width_limit(), Some(300.0));
    assert!(styles[0].grow);
    assert_eq!(styles[1].surface, Surface::None);
    assert_eq!(styles[1].fixed_width(), None);
    assert_eq!(styles[1].width_limit(), None);
    assert!(!styles[1].grow);
}

#[test]
fn a_scroll_area_grows_and_sits_inside_the_window_content() {
    let program = program(&app("", "scroll(spacing: 4) { label(\"a\") }"));
    let [Node::Stack { style, scrolls, .. }] = content(&program) else {
        panic!("the window's content holds one scroll area");
    };
    assert!(*scrolls);
    assert!(style.grow);
    assert!((style.spacing - 4.0).abs() < f32::EPSILON);
    assert_eq!(
        errors(&app("", "scroll(grow: false) { label(\"a\") }")),
        ["`scroll` has no argument named `grow`"]
    );
}

#[test]
fn several_top_level_elements_get_an_implicit_column() {
    let program = program(&app("", "label(\"a\")\nlabel(\"b\")"));
    assert_eq!(content(&program).len(), 2);
}

#[test]
fn ui_depends_on_state_read_through_functions() {
    let program = program(&app(
        "state a = 1\nstate b = 2\nfn twice() -> int { return b * 2 }",
        "label(\"{twice()}\")\nif a > 0 { label(\"positive\") }",
    ));
    let nodes = content(&program);
    assert_eq!(deps(&nodes[0]), [StateId(1)]);
    assert_eq!(deps(&nodes[1]), [StateId(0)]);
}

#[test]
fn constant_text_has_no_dependencies() {
    let program = program(&app("state a = 1", "label(\"hello\")"));
    assert_eq!(deps(&content(&program)[0]), []);
}

#[test]
fn reports_type_mismatches() {
    assert_eq!(
        errors(&app("state count = 0\nfn reset() { count = \"zero\" }", "")),
        ["expected `int`, found `string`"]
    );
}

#[test]
fn suggests_interpolation_for_numbers_in_labels() {
    assert_eq!(
        errors(&app("state count = 0", "label(count)")),
        ["`label` shows text; turn this `int` into text with \"{…}\""]
    );
}

#[test]
fn suggests_interpolation_when_adding_text_and_numbers() {
    assert_eq!(
        errors(&app(
            "state n = 1\nfn f() -> string { return \"n = \" + n }",
            ""
        )),
        [
            "can't use `+` with `string` and `int`; to join text and a value, use interpolation: \"{value}\""
        ]
    );
}

#[test]
fn reports_unknown_names_and_elements() {
    assert_eq!(
        errors(&app("", "label(missing)\nbutton_group()")),
        [
            "unknown name `missing`",
            "unknown element or view `button_group`"
        ]
    );
}

#[test]
fn empty_lists_need_a_type() {
    assert_eq!(
        errors(&app("state items = []", "")),
        ["can't tell what this empty list holds; add a type such as `: [int]`"]
    );
}

#[test]
fn state_starting_values_cannot_use_other_state() {
    assert_eq!(
        errors(&app("state a = 1\nstate b = a + 1", "")),
        ["a state's starting value can't use other state"]
    );
}

#[test]
fn showing_the_ui_cannot_change_state() {
    assert_eq!(
        errors(&app(
            "state n = 0\nfn bump() -> int {\n  n += 1\n  return n\n}",
            "label(\"{bump()}\")"
        )),
        ["showing the UI can't change state; do that in a button's action"]
    );
}

#[test]
fn bindings_need_state() {
    assert_eq!(
        errors(&app(
            "state draft = \"\"\nview Field(text: string) { text_field($text) }",
            "text_field(draft)\nField(text: draft)"
        )),
        [
            "only state, or a `for` variable looping over state, can be bound",
            "`text_field` needs a two-way binding to a `string`, such as `$name`"
        ]
    );
}

#[test]
fn loop_variables_over_state_can_be_bound() {
    program(&app(
        "struct Item { done: bool }\nstate items: [Item] = []",
        "for item in items { checkbox(checked: $item.done) }",
    ));
}

#[test]
fn checks_the_lineup_example() {
    let program = program(include_str!("../../../fixtures/lineup.nf"));
    let nodes = content(&program);
    let Node::For { head, .. } = &nodes[0] else {
        panic!("the lineup starts with a `for`");
    };
    assert!(head.index.is_some());
    assert_eq!(deps(&nodes[0]), [StateId(0)]);
}

#[test]
fn loop_indexes_and_ranges_count_in_ints() {
    assert_eq!(
        errors(&app(
            "state names = [\"a\"]",
            "for index, name in names { label(index) }\nfor n in 0..\"3\" { label(n) }\nfor n in 0.5..2 {}"
        )),
        [
            "`label` shows text; turn this `int` into text with \"{…}\"",
            "expected `int`, found `string`",
            "`label` shows text; turn this `int` into text with \"{…}\"",
            "expected `int`, found `float`",
        ]
    );
}

#[test]
fn loop_counters_cant_be_changed() {
    assert_eq!(
        errors(&app(
            "state names = [\"a\"]\nfn f() {\n  for index, name in names { index += 1 }\n  for n in 0..3 { n = 0 }\n}",
            ""
        )),
        [
            "the loop sets this each time round, so it can't be changed; copy it into a `let` first",
            "the loop sets this each time round, so it can't be changed; copy it into a `let` first",
        ]
    );
}

#[test]
fn a_loop_index_and_item_need_different_names() {
    assert_eq!(
        errors(&app("state names = [\"a\"]", "for name, name in names {}")),
        ["`name` names both the index and the item"]
    );
}

#[test]
fn functions_must_return_on_every_path() {
    assert_eq!(
        errors(&app(
            "fn sign(n: int) -> int {\n  if n > 0 { return 1 }\n}",
            ""
        )),
        ["`sign` can reach its end without returning a `int`"]
    );
}

#[test]
fn an_app_needs_a_window() {
    assert_eq!(
        errors("state count = 0"),
        ["a Nifty app needs a `window(\"Title\") { … }` to show"]
    );
}

#[test]
fn built_in_element_names_are_reserved() {
    assert_eq!(
        errors(&app("fn label() {}", "")),
        ["`label` is a built-in element; choose another name"]
    );
}

#[test]
fn files_of_a_program_share_one_namespace() {
    let files = [
        nifty_syntax::parse("window(\"T\") {\n  Header()\n}", FileId::new(0)),
        nifty_syntax::parse(
            "view Header() {\n  label(title)\n}\nstate title = \"Hi\"",
            FileId::new(1),
        ),
    ];
    let program = compile(&[&files[0], &files[1]]).expect("names from either file resolve in both");
    assert_eq!(program.views[0].name, "Header");
    assert_eq!(program.states[0].name, "title");
}

#[test]
fn a_name_declared_in_two_files_points_at_both() {
    let main = "state count = 0\nwindow(\"T\") {}";
    let other = "fn count() {}";
    let files = [
        nifty_syntax::parse(main, FileId::new(0)),
        nifty_syntax::parse(other, FileId::new(1)),
    ];
    let diagnostics = compile(&[&files[0], &files[1]]).expect_err("`count` is declared twice");
    assert_eq!(diagnostics.len(), 1);
    let [first, second] = [
        Span::new(FileId::new(0), 6, 11),
        Span::new(FileId::new(1), 3, 8),
    ];
    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic.message, "`count` is already declared");
    let places = [diagnostic.span, diagnostic.notes[0].span];
    assert!(
        places.contains(&first) && places.contains(&second),
        "{places:?}"
    );
}

#[test]
fn syntax_errors_from_every_file_are_reported_together() {
    let files = [
        nifty_syntax::parse("window(\"T\") {", FileId::new(0)),
        nifty_syntax::parse("state = 1", FileId::new(1)),
    ];
    let diagnostics = compile(&[&files[0], &files[1]]).expect_err("both files have syntax errors");
    let in_files: Vec<FileId> = diagnostics.iter().map(|d| d.span.file).collect();
    assert_eq!(in_files, [FileId::new(0), FileId::new(1)]);
}

#[test]
fn styles_list_their_options() {
    assert_eq!(
        errors(&app("", "column(align: .middle) {}")),
        ["expected one of `.stretch`, `.start`, `.center`, `.end`"]
    );
}

fn label_style(ui: &str) -> LabelStyle {
    match &content(&program(&app("", ui)))[0] {
        Node::Label { style, .. } => *style,
        other => panic!("expected a label, not {other:?}"),
    }
}

#[test]
fn labels_take_color_alignment_and_selection() {
    assert_eq!(
        label_style("label(\"x\", color: .danger, align: .center, selectable: true)"),
        LabelStyle {
            font: TextStyle::Body,
            color: TextColor::Danger,
            align: TextAlign::Center,
            selectable: true,
        }
    );
}

#[test]
fn a_label_color_defaults_to_its_style() {
    assert_eq!(
        label_style("label(\"x\", style: .caption)").color,
        TextColor::Secondary
    );
    assert_eq!(
        label_style("label(\"x\", style: .caption, color: .primary)").color,
        TextColor::Primary
    );
}

#[test]
fn flags_must_be_written_literally() {
    assert_eq!(
        errors(&app("state on = true", "label(\"x\", selectable: on)")),
        ["this must be `true` or `false`"]
    );
}

#[test]
fn buttons_need_an_action_and_labels_take_no_block() {
    assert_eq!(
        errors(&app("", "button(\"Save\")\nlabel(\"Hi\") { }")),
        [
            "`button` needs a `{ … }` block with what it does",
            "`label` doesn't take a `{ … }` block"
        ]
    );
}

#[test]
fn return_only_works_in_functions() {
    assert_eq!(
        errors(&app("", "button(\"Go\") { return }")),
        ["`return` only works inside a function"]
    );
}

#[test]
fn reports_bad_arguments() {
    assert_eq!(
        errors(&app(
            "fn add(a: int, b: int) -> int { return a + b }\nstate n = 0",
            "button(\"x\") { n = add(1) }\nlabel(\"x\", size: 3)"
        )),
        [
            "`add` needs an argument for `b`",
            "`label` has no argument named `size`"
        ]
    );
}

#[test]
fn a_mismatched_compound_assignment_reports_once() {
    assert_eq!(
        errors(&app("state count = 0\nfn f() { count += \"1\" }", "")),
        ["expected `int`, found `string`"]
    );
}

#[test]
fn compound_assignment_checks_the_operator() {
    assert_eq!(
        errors(&app("state on = false\nfn f() { on += true }", "")),
        ["can't use `+` with `bool` and `bool`"]
    );
}

/// Declarations of everything [`sample`] names: the states its bindings bind, and the
/// window its window names name.
const SAMPLE_DECLARATIONS: &str = "state text = \"\"\nstate flag = false\nstate number = 0\nstate texts: [string] = []\nwindow sample(\"Sample\", open: false) {\n}";

/// A value the checker accepts for a parameter of `kind`, given [`SAMPLE_DECLARATIONS`].
fn sample(kind: ParamKind) -> String {
    match kind {
        // A path from the home folder is text every text parameter takes, an image's too.
        ParamKind::Text | ParamKind::FixedText => "\"~/x\"".to_owned(),
        ParamKind::Number => "number + 1".to_owned(),
        ParamKind::Condition => "!flag".to_owned(),
        ParamKind::TextList => "[\"x\"]".to_owned(),
        ParamKind::Points | ParamKind::FixedNumber => "1".to_owned(),
        ParamKind::Shortcut => "\"shift+n\"".to_owned(),
        ParamKind::Window => "sample".to_owned(),
        ParamKind::Flag => "true".to_owned(),
        ParamKind::Binding(BoundType::String) => "$text".to_owned(),
        ParamKind::Binding(BoundType::Bool) => "$flag".to_owned(),
        ParamKind::Binding(BoundType::Int) => "$number".to_owned(),
        ParamKind::Binding(BoundType::Strings) => "$texts".to_owned(),
        ParamKind::Option(options) => format!(".{}", options.names()[0]),
    }
}

/// A call to `element` with its required arguments, plus whichever others `value` gives a
/// value for.
fn element_call(element: &Element, value: impl Fn(&Param) -> Option<String>) -> String {
    let args: Vec<_> = element
        .params
        .iter()
        .filter_map(|param| {
            let value = value(param).or_else(|| param.required.then(|| sample(param.kind)))?;
            Some(if param.positional {
                value
            } else {
                format!("{}: {value}", param.name)
            })
        })
        .collect();
    let block = match element.block {
        TrailingBlock::None => "",
        TrailingBlock::Children => " { spacer() }",
        TrailingBlock::Action | TrailingBlock::OptionalAction => " { flag = true }",
        TrailingBlock::Menu => " { divider() }",
    };
    format!("{}({}){block}", element.name, args.join(", "))
}

/// `call` where `element` may be written: at the top level beside a window, in a menu, or
/// in a window.
fn element_source(element: &Element, call: &str) -> String {
    if element.name == WINDOW {
        format!("{SAMPLE_DECLARATIONS}\n{call}")
    } else if element.places.contains(&Place::TopLevel) {
        format!("{}\n{call}", app(SAMPLE_DECLARATIONS, ""))
    } else if element.places.contains(&Place::Ui) {
        app(SAMPLE_DECLARATIONS, call)
    } else {
        format!("{}\nmenu(\"M\") {{ {call} }}", app(SAMPLE_DECLARATIONS, ""))
    }
}

#[test]
fn the_element_table_matches_what_the_checker_accepts() {
    for element in ELEMENTS {
        for call in [
            element_call(element, |param| Some(sample(param.kind))),
            element_call(element, |_| None),
        ] {
            let source = element_source(element, &call);
            assert_eq!(errors(&source), Vec::<String>::new(), "{call}");
        }
    }
}

#[test]
fn the_element_table_lists_every_option_the_checker_accepts() {
    for element in ELEMENTS {
        for param in element.params {
            let ParamKind::Option(options) = param.kind else {
                continue;
            };
            for name in options.names() {
                let call = element_call(element, |other| {
                    (other.name == param.name).then(|| format!(".{name}"))
                });
                let source = element_source(element, &call);
                assert_eq!(errors(&source), Vec::<String>::new(), "{call}");
            }
        }
    }
}

/// A call to `function` with every argument, from a button's action.
fn function_source(function: &Function) -> String {
    let args: Vec<_> = function
        .params
        .iter()
        .map(|param| {
            let value = sample(param.kind);
            if param.positional {
                value
            } else {
                format!("{}: {value}", param.name)
            }
        })
        .collect();
    let call = format!("{}({})", function.name, args.join(", "));
    app(SAMPLE_DECLARATIONS, &format!("button(\"Go\") {{ {call} }}"))
}

#[test]
fn the_function_table_matches_what_the_checker_accepts() {
    for function in FUNCTIONS {
        let source = function_source(function);
        assert_eq!(errors(&source), Vec::<String>::new(), "{source}");
    }
}

#[test]
fn the_method_table_matches_what_the_checker_accepts() {
    for method in METHODS {
        let args = vec!["0"; method.params.len()].join(", ");
        let source = app(
            "state numbers: [int] = []",
            &format!(
                "button(\"Go\") {{ let _ = numbers.{}({args}) }}",
                method.name
            ),
        );
        let errors = errors(&source);
        assert!(
            errors.iter().all(|error| !error.contains(method.name)),
            "{source}: {errors:?}"
        );
    }
}

/// Asserts each kind has exactly one table row. `index` must match every kind, so a new
/// kind doesn't compile until it is numbered there, and then fails here until it has a row.
fn assert_one_row_per_kind<K: Copy + std::fmt::Debug>(
    table: &str,
    kinds: impl Iterator<Item = K>,
    count: usize,
    index: fn(K) -> usize,
) {
    let mut rows = vec![0; count];
    for kind in kinds {
        rows[index(kind)] += 1;
    }
    assert!(
        rows.iter().all(|&n| n == 1),
        "{table} rows per kind: {rows:?}"
    );
}

#[test]
fn every_built_in_kind_has_exactly_one_table_row() {
    assert_one_row_per_kind(
        "ELEMENTS",
        ELEMENTS.iter().map(|e| e.kind),
        20,
        |kind| match kind {
            ElementKind::Window => 0,
            ElementKind::Column => 1,
            ElementKind::Row => 2,
            ElementKind::Label => 3,
            ElementKind::Button => 4,
            ElementKind::TextField => 5,
            ElementKind::Checkbox => 6,
            ElementKind::Spacer => 7,
            ElementKind::Divider => 8,
            ElementKind::NavItem => 9,
            ElementKind::Progress => 10,
            ElementKind::Scroll => 11,
            ElementKind::Image => 15,
            ElementKind::Menu => 16,
            ElementKind::MenuItem => 17,
            ElementKind::Switch => 12,
            ElementKind::Slider => 13,
            ElementKind::Picker => 14,
            ElementKind::ContextMenu => 18,
            ElementKind::DropTarget => 19,
        },
    );
    assert_one_row_per_kind(
        "FUNCTIONS",
        FUNCTIONS.iter().map(|f| f.kind),
        7,
        |kind| match kind {
            FunctionKind::Download => 0,
            FunctionKind::PickFile => 1,
            FunctionKind::PickFiles => 2,
            FunctionKind::PickFolder => 3,
            FunctionKind::PickSavePath => 4,
            FunctionKind::OpenWindow => 5,
            FunctionKind::CloseWindow => 6,
        },
    );
    assert_one_row_per_kind(
        "METHODS",
        METHODS.iter().map(|m| m.kind),
        4,
        |kind| match kind {
            MethodKind::Len => 0,
            MethodKind::IsEmpty => 1,
            MethodKind::Push => 2,
            MethodKind::Remove => 3,
        },
    );
}

const DOWNLOAD_STATE: &str = "state percent = 0\nstate status = \"\"";

#[test]
fn download_reports_into_bound_state() {
    let program = program(&app(
        DOWNLOAD_STATE,
        "button(\"Get\") { download(\"https://example.com/a.zip\", to: \"~/Downloads\", progress: $percent, status: $status) }",
    ));
    let Node::Button { action, .. } = &content(&program)[0] else {
        panic!("the window shows a button");
    };
    let [Stmt::Expr(call)] = &action[..] else {
        panic!("the action is one call");
    };
    assert_eq!(call.ty, Type::Void);
    assert!(matches!(call.kind, ExprKind::Download(_)));
}

#[test]
fn download_needs_every_argument() {
    assert_eq!(
        errors(&app(DOWNLOAD_STATE, "button(\"Get\") { download(\"x\") }")),
        [
            "`download` needs a folder to save into, such as `to: \"~/Downloads\"`",
            "`download` needs a two-way binding to a `int`, such as `progress: $name`",
            "`download` needs a two-way binding to a `string`, such as `status: $name`",
        ]
    );
}

#[test]
fn download_binds_a_percentage_and_text() {
    assert_eq!(
        errors(&app(
            DOWNLOAD_STATE,
            "button(\"Get\") { download(\"x\", to: \"y\", progress: $status, status: status) }"
        )),
        [
            "expected `int`, found `string`",
            "`download` needs a two-way binding to a `string`, such as `status: $name`",
        ]
    );
}

#[test]
fn download_changes_state_so_the_ui_cannot_call_it() {
    let fetch = format!(
        "{DOWNLOAD_STATE}\nfn fetch() -> string {{\n  download(\"x\", to: \"y\", progress: $percent, status: $status)\n  return status\n}}"
    );
    assert_eq!(
        errors(&app(&fetch, "label(fetch())")),
        ["showing the UI can't change state; do that in a button's action"]
    );
    assert_eq!(
        errors(&app(&format!("{fetch}\nstate first = fetch()"), "")),
        ["a state's starting value can't use other state"]
    );
}

#[test]
fn built_in_function_names_are_reserved() {
    assert_eq!(
        errors(&app("fn download() {}", "")),
        ["`download` is a built-in function; choose another name"]
    );
    assert_eq!(
        errors(&app(
            "fn f() -> int {\n  let d = download\n  return 0\n}",
            ""
        )),
        [format!(
            "`{DOWNLOAD}` is a built-in function; call it with `{DOWNLOAD}(…)`"
        )]
    );
}

const DIALOG_STATE: &str = "state path = \"\"\nstate paths: [string] = []";

#[test]
fn a_file_dialog_answers_into_bound_state() {
    let program = program(&app(
        DIALOG_STATE,
        "button(\"Open\") { pick_files($paths, title: \"Attach\", types: [\"png\"], from: \"~\") }",
    ));
    let Node::Button { action, .. } = &content(&program)[0] else {
        panic!("the window shows a button");
    };
    let [Stmt::Expr(call)] = &action[..] else {
        panic!("the action is one call");
    };
    assert_eq!(call.ty, Type::Void);
    let ExprKind::Dialog(dialog) = &call.kind else {
        panic!("the call is a file dialog");
    };
    assert_eq!(dialog.kind, DialogKind::Files);
    assert_eq!(dialog.answer.ty, Type::List(std::rc::Rc::new(Type::String)));
}

#[test]
fn a_file_dialog_needs_a_binding_of_its_answer_type() {
    assert_eq!(
        errors(&app(
            DIALOG_STATE,
            "button(\"Open\") {\n  pick_file(path)\n  pick_files($path)\n  pick_folder()\n}"
        )),
        [
            "`pick_file` needs a two-way binding to a `string`, such as `$name`",
            "expected `[string]`, found `string`",
            "`pick_folder` needs a two-way binding to a `string`, such as `$name`",
        ]
    );
}

#[test]
fn only_dialogs_for_files_take_types_and_only_saving_suggests_a_name() {
    assert_eq!(
        errors(&app(
            DIALOG_STATE,
            "button(\"Open\") {\n  pick_folder($path, types: [\"png\"])\n  pick_file($path, name: \"a\")\n  pick_file($path, types: \"png\")\n}"
        )),
        [
            "`pick_folder` has no argument named `types`",
            "`pick_file` has no argument named `name`",
            "expected `[string]`, found `string`",
        ]
    );
}

#[test]
fn a_file_dialog_changes_state_so_the_ui_cannot_call_it() {
    let choose =
        format!("{DIALOG_STATE}\nfn choose() -> string {{\n  pick_file($path)\n  return path\n}}");
    assert_eq!(
        errors(&app(&choose, "label(choose())")),
        ["showing the UI can't change state; do that in a button's action"]
    );
}
