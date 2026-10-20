# Browse the source code

All of Nifty is open source on GitHub:
**[github.com/istorry/nifty](https://github.com/istorry/nifty)**.

Each part below links to two places:

- **Source** opens the code on GitHub.
- **API docs** opens the generated Rust documentation for that crate: every type and
  function, with its doc comments, cross-linked, and a `source` link on each item.

Not sure where to start? Read [How Nifty works](architecture.md) first; it explains how
these pieces fit together.

## The repository at a glance

```text
nifty/
├── crates/              the Rust code that makes up Nifty
│   ├── nifty-syntax/      text → syntax tree
│   ├── nifty-check/       syntax tree → checked program
│   ├── nifty-interp/      runs a checked program
│   ├── nifty-runtime/     widgets and layout, on any OS
│   ├── nifty-platform/    picks the backend for this OS
│   ├── nifty-macos/       the macOS (AppKit) backend
│   ├── nifty-windows/     the Windows (Win32) backend
│   ├── nifty-lsp/         the language server for editors
│   └── nifty-cli/         the `nf` command
├── editors/             editor extensions and grammars
│   ├── vscode/
│   ├── zed/
│   └── tree-sitter-nifty/
├── examples/            example apps
└── docs/                this book
```

## Crates

### `nifty-syntax`: reading source code

Turns text into a syntax tree, with error recovery.
[Source](https://github.com/istorry/nifty/tree/main/crates/nifty-syntax/src) ·
[API docs](../api/nifty_syntax/index.html)

| File | What's in it |
|---|---|
| [`lexer.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-syntax/src/lexer.rs) | splits text into tokens; decides which line breaks matter |
| [`token.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-syntax/src/token.rs) | the token kinds and the keyword list |
| [`parser/`](https://github.com/istorry/nifty/tree/main/crates/nifty-syntax/src/parser) | the parser, split into items, statements and expressions |
| [`ast.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-syntax/src/ast.rs) | the syntax tree types |
| [`diagnostic.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-syntax/src/diagnostic.rs), [`span.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-syntax/src/span.rs) | error messages and source positions, each in one file of the project |
| [`project.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-syntax/src/project.rs) | which `.nf` files make up a project |

### `nifty-check`: understanding the program

Resolves names, checks types, and records what state each part of the UI reads.
[Source](https://github.com/istorry/nifty/tree/main/crates/nifty-check/src) ·
[API docs](../api/nifty_check/index.html)

| File | What's in it |
|---|---|
| [`elements.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-check/src/elements.rs) | the one table of built-in UI elements and their arguments |
| [`functions.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-check/src/functions.rs) | the one table of built-in functions, such as `download` |
| [`methods.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-check/src/methods.rs) | the one table of built-in list and string methods |
| [`hir.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-check/src/hir.rs) | the checked program that the interpreter runs |
| [`checker/`](https://github.com/istorry/nifty/tree/main/crates/nifty-check/src/checker) | the checker: declarations, statements, expressions, UI, arguments, constants |
| [`tests/grammars.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-check/tests/grammars.rs) | checks the editor grammars, docs highlighter and reference lists against those tables |

### `nifty-interp`: running the program

Runs a checked program and keeps the UI in step with state.
[Source](https://github.com/istorry/nifty/tree/main/crates/nifty-interp/src) ·
[API docs](../api/nifty_interp/index.html)

| File | What's in it |
|---|---|
| [`app.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-interp/src/app.rs) | mounts windows, handles events, updates only what changed |
| [`live.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-interp/src/live.rs) | an app whose program can be swapped while it runs, for `nf dev` |
| [`machine/carry.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-interp/src/machine/carry.rs) | which state values survive a reload |
| [`machine/`](https://github.com/istorry/nifty/tree/main/crates/nifty-interp/src/machine) | evaluates statements and expressions; reads and writes places |
| [`value.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-interp/src/value.rs) | runtime values |

### `nifty-runtime`: widgets and layout

The OS-independent widget tree and flexbox layout, plus the `Backend` trait each OS
implements.
[Source](https://github.com/istorry/nifty/tree/main/crates/nifty-runtime/src) ·
[API docs](../api/nifty_runtime/index.html)

| File | What's in it |
|---|---|
| [`ui.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-runtime/src/ui.rs) | the retained UI: create, arrange and update widgets |
| [`backend.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-runtime/src/backend.rs) | the `Backend` trait a platform implements, and the `Waker` other threads use to reach the UI thread |
| [`layout.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-runtime/src/layout.rs) | flexbox layout with Taffy |
| [`style.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-runtime/src/style.rs), [`geometry.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-runtime/src/geometry.rs) | text/button/stack styles; points, sizes, frames |
| [`testing.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-runtime/src/testing.rs) | an in-memory fake backend for tests |

### `nifty-macos`, `nifty-windows` and `nifty-platform`: real native widgets

`nifty-macos` implements `Backend` with AppKit, and `nifty-windows` implements it with
Win32 common controls. Each backend crate compiles to nothing on other systems, so the
workspace builds on both macOS and Windows; CI runs every test on each. `nifty-platform` picks the backend for the
OS you build on. Windows executables need the application manifest that `nifty-cli`'s
`build.rs` embeds, which turns on Common Controls 6 and per-monitor DPI awareness.
[macOS source](https://github.com/istorry/nifty/tree/main/crates/nifty-macos/src) ·
[macOS API docs](../api/nifty_macos/index.html) ·
[Windows source](https://github.com/istorry/nifty/tree/main/crates/nifty-windows/src) ·
[platform source](https://github.com/istorry/nifty/tree/main/crates/nifty-platform/src) ·
[platform API docs](../api/nifty_platform/index.html)

### `nifty-lsp`: editor support

The language server behind `nf lsp`.
[Source](https://github.com/istorry/nifty/tree/main/crates/nifty-lsp/src) ·
[API docs](../api/nifty_lsp/index.html)

| File | What's in it |
|---|---|
| [`server.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-lsp/src/server.rs) | the request loop |
| [`workspace.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-lsp/src/workspace.rs) | open files and the projects they belong to |
| [`project.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-lsp/src/project.rs) | a project's files, analysed together |
| [`document.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-lsp/src/document.rs) | one file's text, syntax tree and position conversion |
| [`index/`](https://github.com/istorry/nifty/tree/main/crates/nifty-lsp/src/index) | an index of every declaration and use, for navigation |
| [`features/`](https://github.com/istorry/nifty/tree/main/crates/nifty-lsp/src/features) | diagnostics, completion, semantic tokens, outline, navigation |

### `nifty-cli`: the `nf` command and `nf-runner`

Two programs: `nf`, the tool you use, and `nf-runner`, which `nf build` puts inside
every app it makes.
[Source](https://github.com/istorry/nifty/tree/main/crates/nifty-cli/src) ·
[API docs](../api/nifty_cli/index.html)

| File | What's in it |
|---|---|
| [`lib.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-cli/src/lib.rs) | the usage text; parses the command line and dispatches |
| [`main.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-cli/src/main.rs), [`bin/nf-runner.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-cli/src/bin/nf-runner.rs) | the two programs' entry points |
| [`create.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-cli/src/create.rs), [`run.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-cli/src/run.rs), [`dev.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-cli/src/dev.rs), [`check.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-cli/src/check.rs), [`build.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-cli/src/build.rs), [`lsp.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-cli/src/lsp.rs) | one file per command |
| [`build/macos.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-cli/src/build/macos.rs) | writes the `.app` bundle and finds the project inside one |
| [`build/windows.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-cli/src/build/windows.rs) | writes the Windows app folder and finds the project inside one |
| [`runner.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-cli/src/runner.rs) | what `nf-runner` does: run the bundled project |
| [`frontend.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-cli/src/frontend.rs) | reads a project and checks it; shared by every command |
| [`report.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-cli/src/report.rs) | prints errors with the source line underneath |
| [`templates/main.nf`](https://github.com/istorry/nifty/blob/main/crates/nifty-cli/templates/main.nf) | the starter app `nf create` writes |

## Editors

| Folder | What's in it |
|---|---|
| [`editors/vscode`](https://github.com/istorry/nifty/tree/main/editors/vscode) | VS Code extension: TextMate grammar + language client |
| [`editors/zed`](https://github.com/istorry/nifty/tree/main/editors/zed) | Zed extension: Tree-sitter queries + language server setup |
| [`editors/tree-sitter-nifty`](https://github.com/istorry/nifty/tree/main/editors/tree-sitter-nifty) | the Tree-sitter grammar |

## This book

The docs you're reading are in
[`docs/`](https://github.com/istorry/nifty/tree/main/docs), written in Markdown and built
with [mdBook](https://rust-lang.github.io/mdBook/). Every page has an edit button at
the top right.
