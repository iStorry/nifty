use super::{Index, Role};

/// Parses error-free source and describes each name as `text role`, where a reference names
/// the offset of the declaration it resolves to.
fn describe(source: &str) -> Vec<String> {
    let parsed = nifty_syntax::parse(source);
    assert_eq!(parsed.diagnostics, []);
    let index = Index::new(&parsed.file);
    index
        .occurrences()
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
    let parsed = nifty_syntax::parse("state count = 0");
    let index = Index::new(&parsed.file);
    let span_at = |offset| index.occurrence_at(offset).map(|o| o.span.range());
    assert_eq!(span_at(5), None);
    assert_eq!(span_at(6), Some(6..11));
    assert_eq!(span_at(11), Some(6..11));
    assert_eq!(span_at(12), None);
}
