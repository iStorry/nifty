//! The editor grammars, the docs highlighter and the language reference keep their own
//! copies of the language's names. These tests check each copy against the tables the
//! compiler uses, so a keyword, type, element, function or method added in Rust fails here
//! until every grammar and reference list has it.
//!
//! Operators are checked in the Tree-sitter grammar and the Zed queries; the `TextMate`
//! grammar matches them with character classes, which its own syntax test covers.

use std::collections::BTreeSet;

use nifty_check::elements::ELEMENTS;
use nifty_check::functions::FUNCTIONS;
use nifty_check::hir::{PRIMITIVE_TYPES, Type};
use nifty_check::methods::METHODS;
use nifty_syntax::{ESCAPES, KEYWORDS, SYMBOLS, TokenKind};
use serde_json::Value;

const TREE_SITTER: &str = include_str!("../../../editors/tree-sitter-nifty/grammar.js");
const ZED: &str = include_str!("../../../editors/zed/languages/nifty/highlights.scm");
const TEXTMATE: &str = include_str!("../../../editors/vscode/syntaxes/nifty.tmLanguage.json");
const DOCS: &str = include_str!("../../../docs/theme/nifty-highlight.js");
const LANGUAGE_REFERENCE: &str = include_str!("../../../docs/src/reference/language.md");
const UI_GUIDE: &str = include_str!("../../../docs/src/guide/ui.md");

/// Literals the grammars match with their own `boolean` rule rather than as keywords.
const BOOLEANS: [TokenKind; 2] = [TokenKind::True, TokenKind::False];

fn spelled(kinds: &[TokenKind]) -> BTreeSet<&'static str> {
    kinds.iter().filter_map(|kind| kind.text()).collect()
}

fn keywords() -> BTreeSet<&'static str> {
    spelled(KEYWORDS)
}

fn keywords_without_booleans() -> BTreeSet<&'static str> {
    &keywords() - &spelled(&BOOLEANS)
}

fn types() -> BTreeSet<&'static str> {
    PRIMITIVE_TYPES
        .iter()
        .filter_map(Type::primitive_name)
        .collect()
}

fn elements() -> BTreeSet<&'static str> {
    ELEMENTS.iter().map(|element| element.name).collect()
}

fn functions() -> BTreeSet<&'static str> {
    FUNCTIONS.iter().map(|function| function.name).collect()
}

fn callable_built_ins() -> BTreeSet<&'static str> {
    &elements() | &functions()
}

fn methods() -> BTreeSet<&'static str> {
    METHODS.iter().map(|method| method.name).collect()
}

fn escapes() -> BTreeSet<char> {
    ESCAPES.iter().copied().collect()
}

/// The names in `wanted` that `file` doesn't write as a double-quoted token.
fn missing_quoted<'a>(file: &str, wanted: impl IntoIterator<Item = &'a str>) -> Vec<&'a str> {
    wanted
        .into_iter()
        .filter(|name| !file.contains(&format!("\"{name}\"")))
        .collect()
}

/// The double-quoted strings between `start` and the next `end` after it.
fn quoted_after<'a>(file: &'a str, start: &str, end: &str) -> BTreeSet<&'a str> {
    let from = file
        .find(start)
        .unwrap_or_else(|| panic!("`{start}` is missing"))
        + start.len();
    let to = from
        + file[from..]
            .find(end)
            .unwrap_or_else(|| panic!("`{end}` doesn't follow `{start}`"));
    file[from..to].split('"').skip(1).step_by(2).collect()
}

/// The characters in a regex class written as `\\[…]`, as both grammars spell escapes.
fn escape_class(file: &str, after: &str) -> BTreeSet<char> {
    let from = file
        .find(after)
        .unwrap_or_else(|| panic!("`{after}` is missing"));
    let class_start = from
        + file[from..]
            .find("\\\\[")
            .unwrap_or_else(|| panic!("no `\\\\[` after `{after}`"))
        + 3;
    let mut chars = file[class_start..].chars();
    let mut class = BTreeSet::new();
    while let Some(c) = chars.next() {
        match c {
            ']' => return class,
            '\\' => {
                class.extend(chars.next());
            }
            other => {
                class.insert(other);
            }
        }
    }
    panic!("the escape class after `{after}` isn't closed")
}

mod tree_sitter {
    use super::*;

    #[test]
    fn has_every_keyword_and_symbol() {
        let wanted = keywords().into_iter().chain(spelled(SYMBOLS));
        assert_eq!(missing_quoted(TREE_SITTER, wanted), Vec::<&str>::new());
    }

    #[test]
    fn accepts_exactly_the_lexer_escapes() {
        assert_eq!(escape_class(TREE_SITTER, "escape_sequence:"), escapes());
    }
}

mod zed {
    use super::*;

    #[test]
    fn highlights_every_keyword_and_symbol() {
        let wanted = keywords_without_booleans()
            .into_iter()
            .chain(spelled(SYMBOLS));
        assert_eq!(missing_quoted(ZED, wanted), Vec::<&str>::new());
    }

    #[test]
    fn lists_exactly_the_keywords() {
        let listed: BTreeSet<_> = ZED
            .match_indices("] @keyword")
            .filter_map(|(end, _)| ZED[..end].rsplit_once('['))
            .flat_map(|(_, list)| list.split('"').skip(1).step_by(2))
            .collect();
        assert_eq!(listed, keywords_without_booleans());
    }

    #[test]
    fn lists_exactly_the_built_in_types() {
        assert_eq!(quoted_after(ZED, "(#any-of? @type.builtin", "))"), types());
    }

    #[test]
    fn lists_exactly_the_built_in_elements_and_functions() {
        assert_eq!(
            quoted_after(ZED, "(#any-of? @function.builtin", "))"),
            callable_built_ins()
        );
    }

    #[test]
    fn lists_exactly_the_built_in_methods() {
        assert_eq!(
            quoted_after(ZED, "(#any-of? @function.method.builtin", "))"),
            methods()
        );
    }
}

mod textmate {
    use super::*;

    fn repository(name: &str) -> Value {
        let grammar: Value = serde_json::from_str(TEXTMATE).expect("the grammar is JSON");
        grammar["repository"][name].clone()
    }

    /// The match of the pattern named `scope` in the repository entry `name`.
    fn pattern(name: &str, scope: &str) -> String {
        repository(name)["patterns"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|pattern| pattern["name"] == scope)
            .and_then(|pattern| pattern["match"].as_str())
            .unwrap_or_else(|| panic!("no `{scope}` pattern in `{name}`"))
            .to_owned()
    }

    /// The words of a pattern written `\b(a|b)\b…` or `\bword\b`.
    fn words(pattern: &str) -> BTreeSet<String> {
        pattern
            .split("\\b")
            .nth(1)
            .unwrap_or_default()
            .trim_matches(['(', ')'])
            .split('|')
            .map(str::to_owned)
            .collect()
    }

    fn owned(names: BTreeSet<&str>) -> BTreeSet<String> {
        names.into_iter().map(str::to_owned).collect()
    }

    #[test]
    fn lists_exactly_the_keywords() {
        let listed: BTreeSet<String> = repository("keyword")["patterns"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|pattern| pattern["match"].as_str())
            .flat_map(words)
            .collect();
        assert_eq!(listed, owned(keywords()));
    }

    #[test]
    fn lists_exactly_the_built_in_types() {
        let types_pattern = pattern("type", "support.type.primitive.nifty");
        assert_eq!(words(&types_pattern), owned(types()));
    }

    #[test]
    fn lists_exactly_the_built_in_elements_and_functions() {
        let calls = pattern("call", "support.function.builtin.nifty");
        assert_eq!(words(&calls), owned(callable_built_ins()));
    }

    #[test]
    fn lists_exactly_the_built_in_methods() {
        let members = repository("member")["patterns"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|pattern| pattern["captures"]["2"]["name"] == "support.function.method.nifty")
            .and_then(|pattern| pattern["match"].as_str())
            .expect("a built-in method pattern")
            .to_owned();
        assert_eq!(words(&members), owned(methods()));
    }

    #[test]
    fn accepts_exactly_the_lexer_escapes() {
        let escape = pattern("string", "constant.character.escape.nifty");
        assert_eq!(escape_class(&escape, ""), escapes());
    }
}

mod docs {
    use super::*;

    fn words_after(start: &str) -> BTreeSet<&'static str> {
        let quoted = quoted_after(DOCS, start, "\n");
        quoted.into_iter().flat_map(str::split_whitespace).collect()
    }

    #[test]
    fn lists_exactly_the_keywords() {
        let listed = &words_after("keyword: ") | &words_after("literal: ");
        assert_eq!(listed, keywords());
    }

    #[test]
    fn lists_exactly_the_built_in_types() {
        assert_eq!(words_after("type: "), types());
    }

    #[test]
    fn lists_exactly_the_built_ins() {
        assert_eq!(quoted_after(DOCS, "const ELEMENTS = [", "]"), elements());
        assert_eq!(quoted_after(DOCS, "const FUNCTIONS = [", "]"), functions());
        assert_eq!(quoted_after(DOCS, "const METHODS = [", "]"), methods());
    }
}

mod reference {
    use super::*;

    /// The text of the first line of `file` starting with `start`, after `start`.
    fn line_after<'a>(file: &'a str, start: &str) -> &'a str {
        file.lines()
            .find_map(|line| line.strip_prefix(start))
            .unwrap_or_else(|| panic!("no line starts with `{start}`"))
    }

    /// The words inside backticks, from a line such as ``The elements are `a`, `b` and `c`.``
    /// or ``Keywords: `a b c`.``.
    fn backticked(text: &str) -> BTreeSet<&str> {
        text.split('`')
            .skip(1)
            .step_by(2)
            .flat_map(str::split_whitespace)
            .collect()
    }

    #[test]
    fn the_language_reference_lists_exactly_the_keywords() {
        let line = line_after(LANGUAGE_REFERENCE, "- Keywords: ");
        assert_eq!(backticked(line), keywords());
    }

    #[test]
    fn the_language_reference_lists_exactly_the_escapes() {
        let line = line_after(LANGUAGE_REFERENCE, "- Strings: ");
        let (_, after) = line.split_once("with escapes `").expect("an escape list");
        let (list, _) = after.split_once('`').expect("the escape list is closed");
        let listed: BTreeSet<char> = list
            .split_whitespace()
            .filter_map(|escape| escape.strip_prefix('\\')?.chars().next())
            .collect();
        assert_eq!(listed, escapes());
    }

    #[test]
    fn the_ui_guide_lists_exactly_the_elements() {
        let start = UI_GUIDE
            .find("The elements are ")
            .expect("the UI guide lists the elements");
        let sentence = &UI_GUIDE[start..start + UI_GUIDE[start..].find(". ").expect("a sentence")];
        assert_eq!(backticked(sentence), elements());
    }
}
