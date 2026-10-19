use crate::check;
use crate::elements::{BoundType, ELEMENTS, Element, Param, ParamKind, TrailingBlock, WINDOW};
use crate::hir::{Node, Program, StateId, Type};

fn parse(source: &str) -> nifty_syntax::Parsed {
    let parsed = nifty_syntax::parse(source);
    assert_eq!(parsed.diagnostics, [], "the test source must parse");
    parsed
}

fn program(source: &str) -> Program {
    let checked = check(&parse(source).file);
    assert_eq!(checked.diagnostics, [], "the test source must check");
    checked.program
}

fn errors(source: &str) -> Vec<String> {
    check(&parse(source).file)
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
    match &program.windows[0].content {
        Node::Stack { children, .. } => children,
        other => panic!("window content is a stack, not {other:?}"),
    }
}

fn deps(node: &Node) -> Vec<StateId> {
    match node {
        Node::Label { text, .. } => text.deps.iter().collect(),
        Node::If { deps, .. } | Node::For { deps, .. } | Node::View { deps, .. } => {
            deps.iter().collect()
        }
        other => panic!("{other:?} has no dependencies"),
    }
}

#[test]
fn checks_the_counter_example() {
    let program = program(include_str!("../../../examples/counter/main.nf"));
    assert_eq!(program.states[0].ty, Type::Int);
    let window = &program.windows[0];
    assert_eq!(window.title, "Counter");
    assert_eq!((window.size.width, window.size.height), (320.0, 200.0));
    assert_eq!(deps(&content(&program)[0]), [StateId(0)]);
}

#[test]
fn checks_the_todos_example() {
    let program = program(include_str!("../../../examples/todos/main.nf"));
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
fn styles_list_their_options() {
    assert_eq!(
        errors(&app("", "column(align: .middle) {}")),
        ["expected one of `.stretch`, `.start`, `.center`, `.end`"]
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

/// A value the checker accepts for a parameter of `kind`, given the state `app_state`
/// declares.
fn sample(kind: ParamKind) -> String {
    match kind {
        ParamKind::Text | ParamKind::FixedText => "\"x\"".to_owned(),
        ParamKind::Points => "1".to_owned(),
        ParamKind::Binding(BoundType::String) => "$text".to_owned(),
        ParamKind::Binding(BoundType::Bool) => "$flag".to_owned(),
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
        TrailingBlock::Action => " { flag = true }",
    };
    format!("{}({}){block}", element.name, args.join(", "))
}

fn element_source(element: &Element, call: &str) -> String {
    let declarations = "state text = \"\"\nstate flag = false";
    if element.name == WINDOW {
        format!("{declarations}\n{call}")
    } else {
        app(declarations, call)
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
