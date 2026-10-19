# Contributing

Thanks for wanting to help! Bug reports, confusing error messages, docs fixes and code
are all welcome.

## Reporting a problem

Open an [issue](https://github.com/istorry/nifty/issues) with:

- the smallest `.nf` file that shows the problem,
- what you expected, and what happened instead (paste the full output of `nf check` or
  `nf run`).

## Building from source

You'll need [Rust](https://rustup.rs) 1.87 or newer, on macOS or Windows.

```bash
git clone https://github.com/istorry/nifty.git
cd nifty
cargo build
```

To put the `nf` you built on your `PATH` in place of a packaged one:

```bash
cargo install --path crates/nifty-cli
```

Run the `nf` you just built without installing it:

```bash
cargo run --bin nf -- run examples/todos
```

## Before you send a change

All three must pass:

```bash
cargo test
cargo clippy --all-targets
cargo fmt --check
```

The workspace turns on Clippy's `pedantic` lints and forbids `unwrap()`, so expect it to
be strict.

## Where things go

| You want to… | Start in |
|---|---|
| change how code is read, or add syntax | `crates/nifty-syntax` (then the [grammars](#keep-the-grammars-in-step)) |
| add or change a type rule or error message | `crates/nifty-check/src/checker` |
| add a UI element | `crates/nifty-check/src/elements.rs`, then the runtime and backend |
| change how apps run | `crates/nifty-interp` |
| change a native widget | `crates/nifty-macos` |
| add an editor feature | `crates/nifty-lsp/src/features` |

See [How Nifty works](architecture.md) for how these fit together.

### Keep the grammars in step

Syntax lives in three places that must agree: the real parser
(`crates/nifty-syntax`), the Tree-sitter grammar (`editors/tree-sitter-nifty`) and the
TextMate grammar (`editors/vscode/syntaxes`). If you change syntax, update all three and
run their tests:

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
- Show example apps with `\{{#include}}` from `examples/` rather than copying them, so
  the docs can't drift from the real code.

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

It needs two repository secrets:

| Secret | What it is |
|---|---|
| `HOMEBREW_TAP_TOKEN` | a fine-grained token with **Contents: read and write** on `istorry/homebrew-tap` |
| `WINGET_TOKEN` | a classic token with the `public_repo` scope; winget pull requests come from its account's fork of `winget-pkgs` |
