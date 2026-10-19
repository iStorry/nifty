# Install Nifty

Nifty comes as one command-line tool called **`nf`**. You use it to create, check and
run apps. This page gets it onto your computer.

> **You'll need:** a Mac (Nifty runs apps on macOS today) and about five minutes.

## Step 1: Install Rust

Nifty is written in [Rust](https://www.rust-lang.org/), and for now you build `nf`
from its source code. If you don't have Rust yet, install it with the official
installer:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

When it finishes, open a **new** terminal window so it picks up the change. Check it
worked:

```bash
cargo --version
```

You should see something like `cargo 1.99.1`. Nifty needs Rust **1.85 or newer**
(it uses the 2024 edition).

## Step 2: Get the Nifty source code

```bash
git clone https://github.com/istorry/nifty.git
cd nifty
```

## Step 3: Build and install `nf`

```bash
cargo install --path crates/nifty-cli
```

This builds `nf` and puts it in `~/.cargo/bin`, which the Rust installer already
added to your `PATH`. The first build downloads a few libraries and takes a minute or
two.

## Step 4: Check it works

```bash
nf help
```

You should see:

```text
usage: nf <command> [args]

commands:
  create <name>      create a new app in a folder called <name>
  run [project]      run an app (default: the current folder)
  check [project]    report errors in a project (default: the current folder)
  lsp                run the language server on stdin and stdout, for editors

A project is a folder with a main.nf file, or a single .nf file.
```

That's it. Next, [make your first app](first-app.md).

## Updating

To get the latest Nifty, pull the newest code and install again:

```bash
git pull
cargo install --path crates/nifty-cli
```

## If something goes wrong

| What you see | What to do |
|---|---|
| `command not found: cargo` | Open a new terminal window after installing Rust. If it still fails, run `source "$HOME/.cargo/env"`. |
| `command not found: nf` | Make sure `~/.cargo/bin` is on your `PATH` (the Rust installer normally does this). |
| `Nifty has a macOS backend only so far` | You're building on Windows or Linux. Those are coming; see [What's next](../roadmap.md). |
