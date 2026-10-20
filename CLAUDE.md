# Nifty — rules for this repo

## A feature is done only when every surface has it

Nifty is one language seen through many surfaces: the checker, the interpreter, two
native backends, the language server, three editor grammars and the docs. A change that
lands in some of them and not the others is a bug, not a partial feature. There is no
"macOS now, Windows later" and no "LSP support in a follow-up".

Before calling any change done, walk the checklist for its kind. Every box is either
done in the same change or impossible for a reason written in the PR.

### New or changed syntax

- [ ] `crates/nifty-syntax` — lexer/parser, with tests
- [ ] `editors/tree-sitter-nifty` — `grammar.js` and a `test/corpus` case
      (`npm run generate && npm test`)
- [ ] `editors/vscode/syntaxes/nifty.tmLanguage.json` and `tests/syntax.test.nf`
      (`npm test`)
- [ ] `editors/zed/languages/nifty/*.scm` — highlights, outline, indents, text objects
- [ ] `crates/nifty-lsp` — semantic tokens, outline, navigation, completion context
- [ ] `docs/src/reference/language.md` and `docs/src/reference/cheat-sheet.md`

### New UI element or element argument

- [ ] `crates/nifty-check/src/elements.rs` — the single table; never describe an
      element anywhere else
- [ ] `crates/nifty-interp` — runs it, with tests
- [ ] `crates/nifty-runtime` — `Backend` method if the element needs a native widget
- [ ] `crates/nifty-macos` **and** `crates/nifty-windows` — both implement it
- [ ] `crates/nifty-lsp` — completion and signature help come from the table; add a
      test that shows them
- [ ] `docs/src/reference/elements.md`, `docs/src/guide/ui.md`, cheat sheet

### New built-in function

- [ ] `crates/nifty-check/src/functions.rs` and `checker/builtins.rs`
- [ ] `crates/nifty-interp` — implementation and tests, on both OSes
- [ ] `crates/nifty-lsp` — completion/signature test (data comes from `FUNCTIONS`)
- [ ] `docs/src/reference/functions.md`, cheat sheet

### New type rule or error message

- [ ] `crates/nifty-check/src/checker` and `tests.rs`
- [ ] the diagnostic has a span the LSP can underline
- [ ] `docs/src/guide/errors.md` when users will meet it

### New `nf` command or flag

- [ ] `crates/nifty-cli`, working on macOS and Windows
- [ ] `docs/src/reference/cli.md`

### Every user-visible change

- [ ] `docs/src/roadmap.md` updated if it was listed there
- [ ] an example in `examples/` when it changes how apps are written, included into
      the docs with `{{#include}}`
- [ ] `cargo test`, `cargo clippy --all-targets`, `cargo fmt --check` pass

## Make the compiler hold the rule

Prefer structure over checklists. When a surface can read from a shared table
(`ELEMENTS`, `FUNCTIONS`) or be forced by a trait (`Backend`), make it do that instead
of duplicating the list, so forgetting a surface fails to compile rather than slipping
through review.
