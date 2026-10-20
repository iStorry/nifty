//! Completing what is being typed, offering only what fits where the cursor is: statements
//! that suit the block, elements in UI, argument labels and values in calls, and types after
//! a `:`.

use std::collections::HashSet;

use lsp_types::{CompletionItem, CompletionItemKind, InsertTextFormat};
use nifty_check::elements::{self, Element, Param, ParamKind, Place, TrailingBlock};
use nifty_check::functions::{self, Function};
use nifty_check::hir::{PRIMITIVE_TYPES, Type};
use nifty_syntax::TokenKind;

use crate::context::{self, BlockKind, Call, Slot};
use crate::index::{Symbol, SymbolKind};
use crate::project::Cursor;
use crate::signature;

const ELEMENT_DETAIL: &str = "element";
const FUNCTION_DETAIL: &str = "built-in function";
const TOP_LEVEL_KEYWORDS: &[TokenKind] = &[
    TokenKind::State,
    TokenKind::Fn,
    TokenKind::View,
    TokenKind::Struct,
];
const CODE_KEYWORDS: &[TokenKind] = &[TokenKind::Let, TokenKind::If, TokenKind::For];
const UI_KEYWORDS: &[TokenKind] = &[TokenKind::If, TokenKind::For];
const VALUE_KEYWORDS: &[TokenKind] = &[TokenKind::True, TokenKind::False];

/// Completions for the name ending at the cursor, or `None` where nothing fits.
pub(crate) fn completions(cursor: Cursor) -> Option<Vec<CompletionItem>> {
    let document = cursor.document();
    let word_start = document.word_start(cursor.offset);
    let items = match context::at(document, word_start).slot {
        Slot::Statement(block) => statement(cursor, block),
        Slot::AfterIf => keywords(&[TokenKind::Else]),
        Slot::Expression => values(cursor),
        Slot::ArgumentStart(call) => argument_start(cursor, &call),
        Slot::ArgumentValue(call) => argument_value(cursor, &call),
        Slot::ImplicitMember(call) => options(cursor, &call)
            .into_iter()
            .map(|name| item(name, CompletionItemKind::ENUM_MEMBER))
            .collect(),
        Slot::Type => types(cursor),
        Slot::ForIn => keywords(&[TokenKind::In]),
        Slot::Nothing => return None,
    };
    let mut seen = HashSet::new();
    Some(
        items
            .into_iter()
            .filter(|item| seen.insert(item.label.clone()))
            .collect(),
    )
}

fn statement(cursor: Cursor, block: BlockKind) -> Vec<CompletionItem> {
    match block {
        BlockKind::TopLevel => {
            let mut items = keywords(TOP_LEVEL_KEYWORDS);
            items.extend(elements_in(Place::TopLevel));
            items
        }
        BlockKind::Menu => elements_in(Place::Menu).collect(),
        BlockKind::Ui => {
            let mut items: Vec<_> = elements_in(Place::Ui).collect();
            items.extend(symbols(cursor, |kind| kind == SymbolKind::View));
            items.extend(keywords(UI_KEYWORDS));
            items
        }
        BlockKind::Function | BlockKind::Action | BlockKind::Other => {
            let mut items: Vec<_> = symbols(cursor, |kind| {
                matches!(
                    kind,
                    SymbolKind::State
                        | SymbolKind::Parameter
                        | SymbolKind::Local
                        | SymbolKind::Function
                )
            })
            .collect();
            items.extend(functions::FUNCTIONS.iter().map(function));
            items.extend(keywords(CODE_KEYWORDS));
            if block == BlockKind::Function {
                items.extend(keywords(&[TokenKind::Return]));
            }
            items
        }
        BlockKind::Struct => Vec::new(),
    }
}

/// The elements that may be written in `place`.
fn elements_in(place: Place) -> impl Iterator<Item = CompletionItem> {
    elements::ELEMENTS
        .iter()
        .filter(move |candidate| candidate.places.contains(&place))
        .map(element)
}

/// Everything an expression can start with.
fn values(cursor: Cursor) -> Vec<CompletionItem> {
    let mut items: Vec<_> = symbols(cursor, |kind| {
        matches!(
            kind,
            SymbolKind::State
                | SymbolKind::Parameter
                | SymbolKind::Local
                | SymbolKind::Function
                | SymbolKind::Struct
        )
    })
    .collect();
    items.extend(keywords(VALUE_KEYWORDS));
    items
}

/// Labels for the parameters not given yet, then values for a positional argument.
fn argument_start(cursor: Cursor, call: &Call) -> Vec<CompletionItem> {
    let Some(signature) = signature_of(cursor, call) else {
        return values(cursor);
    };
    let mut items: Vec<_> = signature
        .params
        .iter()
        .enumerate()
        .filter(|(index, param)| {
            param.named && *index >= call.positional && !call.labels.contains(&param.name)
        })
        .map(|(_, param)| CompletionItem {
            insert_text: Some(format!("{}: ", param.name)),
            detail: Some(param.detail.clone()),
            ..item(&format!("{}:", param.name), CompletionItemKind::PROPERTY)
        })
        .collect();
    if let Some(index) = signature.active(None, call.positional) {
        items.extend(argument_values(cursor, signature.params[index].kind));
    }
    items
}

/// Values for a labelled argument.
fn argument_value(cursor: Cursor, call: &Call) -> Vec<CompletionItem> {
    match labelled_param(cursor, call) {
        Some(param) => argument_values(cursor, param.kind),
        None => values(cursor),
    }
}

/// What fits an argument: anything for a declared callable's parameter, and for an element
/// parameter only what the element accepts there.
fn argument_values(cursor: Cursor, kind: Option<ParamKind>) -> Vec<CompletionItem> {
    match kind {
        None | Some(ParamKind::Condition) => values(cursor),
        Some(ParamKind::Text | ParamKind::TextList | ParamKind::Number) => {
            symbols(cursor, |kind| {
                matches!(
                    kind,
                    SymbolKind::State
                        | SymbolKind::Parameter
                        | SymbolKind::Local
                        | SymbolKind::Function
                )
            })
            .collect()
        }
        Some(
            ParamKind::FixedText | ParamKind::Points | ParamKind::FixedNumber | ParamKind::Shortcut,
        ) => Vec::new(),
        Some(ParamKind::Flag) => keywords(VALUE_KEYWORDS),
        Some(ParamKind::Window) => symbols(cursor, |kind| kind == SymbolKind::Window).collect(),
        Some(ParamKind::Binding(_)) => symbols(cursor, |kind| {
            matches!(kind, SymbolKind::State | SymbolKind::Local)
        })
        .map(|binding| CompletionItem {
            label: format!("${}", binding.label),
            ..binding
        })
        .collect(),
        Some(ParamKind::Option(options)) => options
            .names()
            .into_iter()
            .map(|name| item(&format!(".{name}"), CompletionItemKind::ENUM_MEMBER))
            .collect(),
    }
}

/// The `.name` values the labelled element argument takes, if it takes a set of them.
fn options(cursor: Cursor, call: &Call) -> Vec<&'static str> {
    match labelled_param(cursor, call).and_then(|param| param.kind) {
        Some(ParamKind::Option(options)) => options.names(),
        _ => Vec::new(),
    }
}

/// The parameter the current label names, when the callee and label are known.
fn labelled_param(cursor: Cursor, call: &Call) -> Option<signature::Param> {
    let mut signature = signature_of(cursor, call)?;
    let index = signature.active(call.label.as_deref(), call.positional)?;
    Some(signature.params.swap_remove(index))
}

fn signature_of(cursor: Cursor, call: &Call) -> Option<signature::Signature> {
    signature::of(cursor.project, call.callee.as_deref()?)
}

fn types(cursor: Cursor) -> Vec<CompletionItem> {
    let primitives = PRIMITIVE_TYPES
        .iter()
        .filter_map(Type::primitive_name)
        .map(|name| item(name, CompletionItemKind::TYPE_PARAMETER));
    let structs = symbols(cursor, |kind| kind == SymbolKind::Struct);
    primitives.chain(structs).collect()
}

fn symbols(
    cursor: Cursor,
    wanted: impl Fn(SymbolKind) -> bool,
) -> impl Iterator<Item = CompletionItem> {
    cursor
        .project
        .index
        .in_scope(cursor.file, cursor.offset)
        .filter(move |symbol| wanted(symbol.kind))
        .map(symbol_item)
}

fn symbol_item(symbol: &Symbol) -> CompletionItem {
    let kind = match symbol.kind {
        SymbolKind::State | SymbolKind::Parameter | SymbolKind::Local => {
            CompletionItemKind::VARIABLE
        }
        SymbolKind::Struct => CompletionItemKind::STRUCT,
        SymbolKind::Field => CompletionItemKind::FIELD,
        SymbolKind::Function => CompletionItemKind::FUNCTION,
        SymbolKind::View => CompletionItemKind::CLASS,
        SymbolKind::Window => CompletionItemKind::CONSTANT,
    };
    item(&symbol.name, kind)
}

/// An element, inserted with placeholders for its required arguments and its block.
fn element(element: &Element) -> CompletionItem {
    let block = match element.block {
        TrailingBlock::None | TrailingBlock::OptionalAction => "$0",
        TrailingBlock::Children | TrailingBlock::Action | TrailingBlock::Menu => " {\n\t$0\n}",
    };
    call(
        element.name,
        element.params,
        block,
        element.summary,
        ELEMENT_DETAIL,
    )
}

/// A built-in function, inserted with placeholders for its required arguments.
fn function(function: &Function) -> CompletionItem {
    call(
        function.name,
        function.params,
        "$0",
        function.summary,
        FUNCTION_DETAIL,
    )
}

/// A built-in call, with numbered placeholders for its required arguments, then `tail`.
fn call(name: &str, params: &[Param], tail: &str, summary: &str, detail: &str) -> CompletionItem {
    let args: Vec<_> = params
        .iter()
        .filter(|param| param.required)
        .enumerate()
        .map(|(index, param)| {
            let placeholder = format!("${{{}:{}}}", index + 1, param.name);
            if param.positional {
                placeholder
            } else {
                format!("{}: {placeholder}", param.name)
            }
        })
        .collect();
    CompletionItem {
        detail: Some(detail.to_owned()),
        documentation: Some(lsp_types::Documentation::String(summary.to_owned())),
        insert_text: Some(format!("{name}({}){tail}", args.join(", "))),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        ..item(name, CompletionItemKind::FUNCTION)
    }
}

fn keywords(keywords: &[TokenKind]) -> Vec<CompletionItem> {
    keywords
        .iter()
        .filter_map(|keyword| keyword.text())
        .map(|keyword| item(keyword, CompletionItemKind::KEYWORD))
        .collect()
}

fn item(label: &str, kind: CompletionItemKind) -> CompletionItem {
    CompletionItem {
        label: label.to_owned(),
        kind: Some(kind),
        ..CompletionItem::default()
    }
}

#[cfg(test)]
mod tests {
    use nifty_check::elements::Options;

    use super::*;
    use crate::project::Project;

    /// The elements that may be written in `place`.
    fn elements_at(place: Place) -> Vec<&'static str> {
        elements::ELEMENTS
            .iter()
            .filter(|element| element.places.contains(&place))
            .map(|element| element.name)
            .collect()
    }

    fn ui_elements() -> Vec<&'static str> {
        elements_at(Place::Ui)
    }

    fn function_names() -> Vec<&'static str> {
        functions::FUNCTIONS
            .iter()
            .map(|function| function.name)
            .collect()
    }

    const DECLARATIONS: &str = "state count = 0\nstruct Todo { title: string }\n\
                                fn bump(by: int) {}\nview Badge(text: string) {}\n";

    /// The labels offered where `|` stands in `source`, which is removed before parsing.
    fn labels(source: &str) -> Vec<String> {
        let (project, offset) = Project::with_cursor(source);
        completions(project.cursor(offset))
            .unwrap_or_default()
            .into_iter()
            .map(|item| item.label)
            .collect()
    }

    fn offers(source: &str, expected: &[&str]) {
        let found = labels(source);
        for label in expected {
            assert!(
                found.iter().any(|found| found == label),
                "`{label}` missing from {found:?}"
            );
        }
    }

    fn omits(source: &str, unexpected: &[&str]) {
        let found = labels(source);
        for label in unexpected {
            assert!(
                !found.iter().any(|found| found == label),
                "`{label}` offered in {found:?}"
            );
        }
    }

    #[test]
    fn a_ui_block_offers_elements_views_if_and_for_only() {
        let source = format!("{DECLARATIONS}window(\"App\") {{\n  r|\n}}");
        offers(&source, &ui_elements());
        offers(&source, &["Badge", "if", "for"]);
        omits(&source, &function_names());
        omits(
            &source,
            &[
                "return",
                "let",
                elements::WINDOW,
                "count",
                "bump",
                "true",
                "struct",
            ],
        );
    }

    #[test]
    fn a_menu_offers_only_what_goes_in_a_menu() {
        let source = format!("{DECLARATIONS}menu(\"File\") {{\n  m|\n}}");
        offers(&source, &elements_at(Place::Menu));
        omits(&source, &["label", "window", "if", "for", "Badge"]);
    }

    #[test]
    fn opening_a_window_offers_the_windows_names() {
        let source = "window main(\"Main\") {\n  button(\"Go\") { open_window(|) }\n}\nwindow settings(\"Settings\", open: false) {\n}";
        offers(source, &["main", "settings"]);
        omits(source, &["count", "true"]);
    }

    #[test]
    fn a_context_menu_offers_what_goes_in_a_menu() {
        let source =
            format!("{DECLARATIONS}window(\"App\") {{\n  context_menu() {{\n    m|\n  }}\n}}");
        offers(&source, &elements_at(Place::Menu));
        omits(&source, &["label", "context_menu", "if", "for", "Badge"]);
    }

    #[test]
    fn the_top_level_offers_declarations_and_window() {
        offers("w|", &["window", "menu", "state", "fn", "view", "struct"]);
        omits("w|", &["row", "return", "let", "if"]);
    }

    #[test]
    fn functions_offer_statements_and_actions_skip_return() {
        let function = format!("{DECLARATIONS}fn f() {{\n  |\n}}");
        offers(&function, &["let", "if", "for", "return", "count", "bump"]);
        offers(&function, &function_names());
        omits(&function, &ui_elements());
        omits(&function, &["Badge", "state"]);
        let action = format!("{DECLARATIONS}window(\"App\") {{\n  button(\"Go\") {{ |}}\n}}");
        offers(&action, &["count", "bump", "let"]);
        offers(&action, &function_names());
        omits(&action, &["return", "row"]);
    }

    #[test]
    fn an_argument_list_offers_the_labels_not_given_yet() {
        offers("window(|", &["width:", "height:"]);
        omits("window(|", &["title:", "row", "return", "true"]);
        let binding = "state draft = \"\"\nwindow(\"App\") {\n  text_field(|";
        offers(binding, &["$draft", "placeholder:"]);
        let after_width = labels("window(\"App\", width: 300, |");
        assert!(after_width.contains(&"height:".to_owned()));
        assert!(!after_width.contains(&"width:".to_owned()));
        offers(&format!("{DECLARATIONS}bump(|"), &["by:", "count"]);
        let download = format!("{DECLARATIONS}fn f() {{\n  download(\"x\", |");
        offers(
            &download,
            &[
                "to:",
                "progress:",
                "status:",
                "received:",
                "total:",
                "speed:",
            ],
        );
        offers(
            &format!("{DECLARATIONS}fn f() {{\n  download(\"x\", progress: |"),
            &["$count"],
        );
    }

    #[test]
    fn element_settings_offer_their_options() {
        let source = "window(\"App\") {\n  row(align: |";
        let alignments: Vec<_> = Options::Alignment
            .names()
            .iter()
            .map(|name| format!(".{name}"))
            .collect();
        assert_eq!(labels(source), alignments);
        assert_eq!(
            labels("window(\"App\") {\n  label(\"x\", style: .|"),
            Options::TextStyle.names()
        );
        assert_eq!(
            labels("window(\"App\") {\n  label(\"x\", align: .|"),
            Options::TextAlignment.names()
        );
        assert_eq!(
            labels("window(\"App\") {\n  label(\"x\", selectable: |"),
            ["true", "false"]
        );
    }

    #[test]
    fn expressions_offer_values_in_scope() {
        let source = format!("{DECLARATIONS}fn f() {{\n  let x = |");
        offers(&source, &["count", "bump", "Todo", "true"]);
        omits(&source, &["Badge", "row", "let"]);
    }

    #[test]
    fn hides_locals_outside_their_block() {
        omits(
            "fn f() {\n  for item in [1] { }\n  let later = 1\n}\nfn g() {\n  |\n}",
            &["item", "later"],
        );
    }

    #[test]
    fn offers_types_after_a_colon_and_nothing_where_a_name_is_declared() {
        let types = format!("{DECLARATIONS}state todos: [|");
        let primitives: Vec<_> = PRIMITIVE_TYPES
            .iter()
            .filter_map(Type::primitive_name)
            .collect();
        offers(&types, &primitives);
        offers(&types, &["Todo"]);
        assert_eq!(labels("state |"), Vec::<String>::new());
        assert_eq!(labels("fn f(a|"), Vec::<String>::new());
        assert_eq!(labels("state s = 1\nfn f() { s.|"), Vec::<String>::new());
    }

    #[test]
    fn offers_nothing_in_strings_or_comments() {
        assert_eq!(labels("window(\"Ap|\")"), Vec::<String>::new());
        assert_eq!(labels("// wi|"), Vec::<String>::new());
    }

    #[test]
    fn offers_else_after_an_if_block_and_in_after_a_for_item() {
        assert_eq!(labels("fn f() {\n  if true { } |"), ["else"]);
        assert_eq!(labels("fn f() {\n  for item |"), ["in"]);
        assert_eq!(labels("fn f() {\n  for index, item |"), ["in"]);
        assert_eq!(labels("fn f() {\n  for index, |"), Vec::<String>::new());
    }
}
