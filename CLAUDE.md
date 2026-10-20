# Nifty — rules for this repo

## A feature is done only when every surface has it

Nifty is one language seen through many surfaces: the checker, the interpreter, two
native backends, the language server, four highlighters and the docs. A change that
reaches some of them and not the others is a bug, not a partial feature. There is no
"macOS now, Windows later" and no "LSP support in a follow-up". Where a platform truly
can't do something yet, the docs say so in the same change.

Most surfaces are held in step by the compiler or a test, so forgetting one fails
loudly. The checklists below say which, so you know what still needs your attention.

### Where each fact lives

| Fact | Its one owner | Read by |
|---|---|---|
| Keywords, punctuation, operators | `TokenKind::text`, `KEYWORDS`, `SYMBOLS` in `nifty-syntax/src/token.rs` | lexer, parser errors, LSP completion, grammar test |
| String escapes | `ESCAPES` in `nifty-syntax/src/lexer.rs` | lexer, `string_literal`, grammar test |
| Built-in types | `Type::primitive_name`, `PRIMITIVE_TYPES` in `nifty-check/src/hir.rs` | checker, LSP, grammar test |
| UI elements | `ELEMENTS` in `nifty-check/src/elements.rs` | checker dispatch, LSP, grammar test |
| Built-in functions | `FUNCTIONS` in `nifty-check/src/functions.rs` | checker dispatch, LSP, grammar test |
| List and string methods | `METHODS` in `nifty-check/src/methods.rs` | checker dispatch, LSP, grammar test |
| Widget operations | the `Backend` trait in `nifty-runtime/src/backend.rs` | `nifty-macos`, `nifty-windows`, `FakeBackend` |

Never write one of these lists anywhere else in Rust. Read the table.

### What holds each surface in step

- **Checker ↔ tables:** the checker dispatches on each entry's `kind`, `Args::built_in`
  asserts every argument it reads is declared, and `nifty-check/src/tests.rs` checks every
  table entry is accepted and every kind has exactly one row.
- **Backends:** every widget operation is a `Backend` method, so both backends must
  implement it to compile. CI builds and tests on `macos-latest` and `windows-latest`.
- **Editor grammars and docs lists:** `nifty-check/tests/grammars.rs` checks the
  Tree-sitter grammar, Zed queries, TextMate grammar, docs highlighter, the keyword and
  escape lists in `docs/src/reference/language.md` and the element list in
  `docs/src/guide/ui.md` against the tables.
- **LSP:** completion, signature help, hover and semantic tokens read the tables, and
  their tests take names from them.

### Still on you

These have no automatic check. Walk them for every user-visible change:

- [ ] **Behaviour on both OSes.** Compiling isn't the same as working: when a backend
      changes, say in the PR what was checked by hand on macOS and on Windows.
- [ ] **Grammar test cases.** New syntax gets a case in
      `editors/tree-sitter-nifty/test/corpus` and `editors/vscode/tests/syntax.test.nf`;
      a new element, function or method gets one too.
- [ ] **Zed grammar pin.** After a Tree-sitter change lands on `main`, bump `rev` in
      `editors/zed/extension.toml` to that commit.
- [ ] **Reference docs.** `docs/src/reference/elements.md`, `functions.md`, `language.md`,
      `cli.md` and `cheat-sheet.md` describe every setting, with both platforms' behaviour
      where it differs.
- [ ] **Fixtures.** A change to how apps are written gets a program in `fixtures/` that
      a test checks; docs that show a whole app `{{#include}}` it rather than copying.
- [ ] **Roadmap.** Move the item in `docs/src/roadmap.md` from planned to working.

### Before calling it done

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace    # the link test links against the runtime library this builds
cargo test --workspace
cd editors/tree-sitter-nifty && npm run generate && npm test
cd editors/vscode && npm test
```

To lint the Windows backend from a Mac, add `--target x86_64-pc-windows-msvc` to the
clippy command. Its tests still only run in CI.
