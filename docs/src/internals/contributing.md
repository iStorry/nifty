# Contributing

Thanks for wanting to help! Bug reports, confusing error messages, docs fixes and code
are all welcome.

## Reporting a problem

Open an [issue](https://github.com/istorry/nifty/issues) with:

- the smallest `.nf` file that shows the problem,
- what you expected, and what happened instead (paste the full output of `nf check` or
  `nf run`).

## Building from source

You'll need [Rust](https://rustup.rs) 1.87 or newer, on macOS or Windows. On Windows,
use the MSVC toolchain (the default from rustup).

```bash
git clone https://github.com/istorry/nifty.git
cd nifty
cargo build
```

To put the `nf` you built on your `PATH` in place of a packaged one, install it, then
copy the runtime library apps are linked against next to it:

```bash
cargo install --path crates/nifty-cli
cargo build --release -p nifty-rt
cp target/release/libnifty_rt.a ~/.cargo/bin/
```

On Windows, the library is `target\release\nifty_rt.lib`. On macOS, `make install` does
all three.

Run the `nf` you just built without installing it:

```bash
cargo run --bin nf -- run fixtures/todos.nf
```

## Before you send a change

A feature is done only when every part of Nifty has it: the checker, the interpreter,
both the macOS and Windows backends, the language server, the editor grammars and the
docs. [`CLAUDE.md`](https://github.com/istorry/nifty/blob/main/CLAUDE.md) has the full
checklist, says which parts tests already keep in step, and lists the commands to run.

These must pass, and CI runs them on macOS and Windows for every pull request:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
cargo test --workspace
```

The build comes first because the link test links against the runtime library it leaves
in `target/debug`.

The workspace turns on Clippy's `pedantic` lints and forbids `unwrap()`, so expect it to
be strict.

## Where things go

| You want to… | Start in |
|---|---|
| change how code is read, or add syntax | `crates/nifty-syntax` (then the [grammars](#keep-the-grammars-in-step)) |
| add or change a type rule or error message | `crates/nifty-check/src/checker` |
| add a UI element | `crates/nifty-check/src/elements.rs`, then the runtime and both backends |
| add a built-in function or method | `crates/nifty-check/src/functions.rs` or `methods.rs`, then `crates/nifty-interp` |
| change how apps run | `crates/nifty-interp` |
| change a native widget | `crates/nifty-macos` and `crates/nifty-windows` |
| add an editor feature | `crates/nifty-lsp/src/features` |

See [How Nifty works](architecture.md) for how these fit together.

### Keep the grammars in step

Syntax lives in the real parser (`crates/nifty-syntax`) and in four highlighters: the
Tree-sitter grammar (`editors/tree-sitter-nifty`, which Zed's queries in
`editors/zed/languages` use), the TextMate grammar (`editors/vscode/syntaxes`) and the
docs highlighter (`docs/theme/nifty-highlight.js`). `cargo test` fails when one of them
is missing a keyword, type, element, function or method, and names it. Add a test case
for new syntax to each grammar and run their tests:

```bash
cd editors/tree-sitter-nifty && npm run generate && npm test
cd editors/vscode && npm test
```

## Working on these docs

The docs are an [mdBook](https://rust-lang.github.io/mdBook/) in `docs/`. To preview
them while you edit:

```bash
cargo install mdbook --version 0.5.4 --locked
mdbook serve docs --open
```

The page reloads as you save. A few conventions:

- Write for someone new to programming. Short sentences, one idea at a time, and an
  example for every rule.
- Mark Nifty code blocks with `nf` so they get syntax colors.
- Show a whole app with `\{{#include}}` from `fixtures/` rather than copying it, so the
  docs can't drift from code the tests check.

Pushing to `main` publishes the docs to GitHub Pages automatically, together with the
API docs from `cargo doc`.

## Releasing

Releases are built by [`release.yml`](https://github.com/istorry/nifty/blob/main/.github/workflows/release.yml).
Bump `version` in the root `Cargo.toml`, merge it, then push a matching tag:

```bash
git tag v0.2.0
git push origin v0.2.0
```

A version with a pre-release part, such as `0.1.0-alpha.1` (tag `v0.1.0-alpha.1`), is
published as a GitHub pre-release.

The workflow builds `nf` and `nf-runner` for macOS (one universal binary) and Windows
(x64 and Arm64), attaches them to a GitHub release, then:

- commits a new `Formula/nifty-cli.rb` to the
  [`istorry/homebrew-tap`](https://github.com/istorry/homebrew-tap) repository, made by
  `packaging/homebrew/formula.sh`;
- opens a pull request on [`microsoft/winget-pkgs`](https://github.com/microsoft/winget-pkgs)
  with the manifests from `packaging/winget/manifests.ps1`. A winget moderator merges it,
  usually within a few days.

The winget step is its own workflow, `.github/workflows/winget.yml`. If it fails, fix the
cause on `main` and resubmit the existing release without tagging again:

```bash
gh workflow run winget.yml -f version=0.1.0-alpha.1
```

It needs two repository secrets:

| Secret | What it is |
|---|---|
| `HOMEBREW_TAP_TOKEN` | a fine-grained token with **Contents: read and write** on `istorry/homebrew-tap` |
| `WINGET_TOKEN` | a classic token with the `public_repo` scope; winget pull requests come from its account's fork of `winget-pkgs` |
