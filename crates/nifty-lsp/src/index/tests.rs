use nifty_syntax::{FileId, Span};

use super::{Index, Role, SymbolKind};

/// Parses error-free source and describes each name as `text role`, where a reference names
/// the offset of the declaration it resolves to.
fn describe(source: &str) -> Vec<String> {
    let parsed = nifty_syntax::parse(source, FileId::default());
    assert_eq!(parsed.diagnostics, []);
    let index = Index::new(&[&parsed.file]);
    index
        .occurrences_in(FileId::default())
        .iter()
        .map(|occurrence| {
            let text = &source[occurrence.span.range()];
            let role = match occurrence.role {
                Role::Declaration(id) => format!("declares {:?}", index.symbol(id).kind),
                Role::Reference(id) => {
                    let declaration = index.declaration_of(id).map(|span| span.start);
                    format!("refers to {:?}@{declaration:?}", index.symbol(id).kind)
                }
                Role::Unresolved(kind) => format!("unresolved {kind:?}"),
            };
            format!("{text} {role}")
        })
        .collect()
}

#[test]
fn resolves_top_level_names_declared_later() {
    assert_eq!(
        describe("fn a() { b() }\nfn b() {}"),
        [
            "a declares Function",
            "b refers to Function@Some(18)",
            "b declares Function"
        ]
    );
}

#[test]
fn locals_shadow_state_from_their_declaration_onwards() {
    assert_eq!(
        describe("state x = 1\nfn f() {\n  let x = x\n  x += 1\n}"),
        [
            "x declares State",
            "f declares Function",
            "x declares Local",
            "x refers to State@Some(6)",
            "x refers to Local@Some(27)"
        ]
    );
}

#[test]
fn loop_items_are_scoped_to_the_loop() {
    assert_eq!(
        describe("state items = [1]\nfn f() {\n  for item in items { item }\n  item\n}"),
        [
            "items declares State",
            "f declares Function",
            "item declares Local",
            "items refers to State@Some(6)",
            "item refers to Local@Some(33)",
            "item unresolved Variable"
        ]
    );
}

#[test]
fn loop_indexes_and_range_bounds_resolve() {
    assert_eq!(
        describe("state n = 1\nfn f() {\n  for i, x in [n] { i }\n  for k in 0..n { k }\n}"),
        [
            "n declares State",
            "f declares Function",
            "i declares Local",
            "x declares Local",
            "n refers to State@Some(6)",
            "i refers to Local@Some(27)",
            "k declares Local",
            "n refers to State@Some(6)",
            "k refers to Local@Some(51)"
        ]
    );
}

#[test]
fn argument_labels_refer_to_fields_and_parameters() {
    let source = "struct P { x: int }\nview V(p: P) {}\nV(p: P(x: 1))";
    assert_eq!(
        describe(source),
        [
            "P declares Struct",
            "x declares Field",
            "int unresolved Type",
            "V declares View",
            "p declares Parameter",
            "P refers to Struct@Some(7)",
            "V refers to View@Some(25)",
            "p refers to Parameter@Some(27)",
            "P refers to Struct@Some(7)",
            "x refers to Field@Some(11)"
        ]
    );
}

#[test]
fn built_ins_fields_and_members_stay_unresolved() {
    assert_eq!(
        describe("state n = 0\nrow(align: .center) { label(n.text, n.len()) }"),
        [
            "n declares State",
            "row unresolved Function",
            "center unresolved ImplicitMember",
            "label unresolved Function",
            "n refers to State@Some(6)",
            "text unresolved Field",
            "n refers to State@Some(6)",
            "len unresolved Method"
        ]
    );
}

#[test]
fn finds_the_name_at_a_cursor_including_just_past_its_end() {
    let parsed = nifty_syntax::parse("state count = 0", FileId::default());
    let index = Index::new(&[&parsed.file]);
    let span_at = |offset| {
        index
            .occurrence_at(FileId::default(), offset)
            .map(|o| o.span.range())
    };
    assert_eq!(span_at(5), None);
    assert_eq!(span_at(6), Some(6..11));
    assert_eq!(span_at(11), Some(6..11));
    assert_eq!(span_at(12), None);
}

#[test]
fn resolves_names_declared_in_other_files() {
    let [main, header] = [
        ("window(\"T\") {\n  Header()\n}", FileId::new(0)),
        (
            "view Header() {\n  let x = 1\n  label(\"{x}\")\n}",
            FileId::new(1),
        ),
    ]
    .map(|(source, file)| nifty_syntax::parse(source, file).file);
    let index = Index::new(&[&main, &header]);

    let call = index
        .occurrence_at(FileId::new(0), 17)
        .expect("`Header` is used in main");
    let Role::Reference(id) = call.role else {
        panic!("`Header` resolves, found {:?}", call.role);
    };
    assert_eq!(
        index.declaration_of(id),
        Some(Span::new(FileId::new(1), 5, 11))
    );

    let local_in = |file| {
        index
            .in_scope(file, 40)
            .any(|symbol| symbol.kind == SymbolKind::Local)
    };
    assert!(local_in(FileId::new(1)));
    assert!(!local_in(FileId::new(0)));
}

#[test]
fn window_names_are_declared_and_referred_to() {
    assert_eq!(
        describe("fn show() { open_window(prefs) }\nwindow prefs(\"Prefs\") {}"),
        [
            "show declares Function",
            "open_window unresolved Function",
            "prefs refers to Window@Some(40)",
            "window unresolved Function",
            "prefs declares Window"
        ]
    );
}
