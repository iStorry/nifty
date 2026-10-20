# Nifty

[![Release](https://img.shields.io/github/v/release/iStorry/nifty?include_prereleases&label=release)](https://github.com/iStorry/nifty/releases)
[![Docs](https://img.shields.io/github/actions/workflow/status/iStorry/nifty/docs.yml?branch=main&label=docs)](https://istorry.github.io/nifty/)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows-lightgrey)](#get-started)
[![App size](https://img.shields.io/badge/app%20size-1.2%20MB-brightgreen)](#smaller-than-a-photo)
[![Homebrew](https://img.shields.io/badge/brew-istorry%2Ftap%2Fnifty--cli-orange?logo=homebrew)](#get-started)

Nifty is a small, friendly programming language for building real desktop apps, with
real buttons, text fields and windows from your operating system.

```nf
state count = 0

window("Counter", width: 320, height: 200) {
  column(spacing: 12, align: .center, justify: .center) {
    label("Clicked {count} times", style: .title)
    row(spacing: 8) {
      button("−") { count -= 1 }
      button("+", style: .primary) { count += 1 }
    }
  }
}
```

That's the whole program. When you click **+**, `count` changes and the label that shows
it updates by itself.

- **Tiny:** a finished macOS app is about 1.2 MB, and about 520 KB zipped.
- **Light:** a running app uses about 18 MB of memory.
- **Native:** apps use the system's own widgets, fonts, accent color and dark mode.
- **Live:** `nf dev` reloads your app each time you save, keeping its state.
- **Checked:** mistakes are caught before the app runs, and explained in plain words.
- **Easy to learn:** you can learn the whole language in an afternoon.

Nifty is at version 0.1.0-alpha.3, an early alpha. It runs and builds apps on macOS and
Windows; Linux comes next.

## Smaller than a photo

`nf build` compiles your project to machine code and turns it into a real app: a `.app`
on macOS, or a folder with an `.exe` on Windows. Here's the to-do app from the [tutorial](https://istorry.github.io/nifty/tutorials/todos.html) after it's built on a Mac:

| | |
|---|---|
| App size | 1.5 MB |
| Zipped, ready to send | 710 KB |
| Memory while running | 13 MB |
| Needs installing first | nothing |

That's smaller than most photos on your phone. There's no browser engine inside and no
runtime to install, because the app talks straight to the operating system. Zip it, send
it, and it's on someone else's computer in a second.

The numbers are from an Apple Silicon Mac. Memory is the figure Activity Monitor shows.

## Why Nifty

One day I was building an app for my wife, and I wanted it to be cross-platform. I didn't
want to use Tauri or Electron, though. They build the UI out of a web page, so the apps
look out of place on the desktop and use a lot of memory. I wanted something light.

So here it is.

## Get started

Install `nf`, the Nifty tool. On macOS, use [Homebrew](https://brew.sh):

```bash
brew install istorry/tap/nifty-cli
```

On Windows, use winget:

```bash
winget install nifty-cli
```

Then make an app, run it, and build it into an app you can share:

```bash
nf create hello
nf dev hello
nf build hello
```

The [Nifty book](https://istorry.github.io/nifty/) has the
[install guide](https://istorry.github.io/nifty/getting-started/install.html),
tutorials, the language guide and the full reference.

## Building from source

You'll need [Rust](https://rustup.rs) 1.87 or newer.

```bash
git clone https://github.com/istorry/nifty.git
cd nifty
cargo build
cargo test --workspace
```

[Contributing](https://istorry.github.io/nifty/internals/contributing.html) explains how
the code is laid out and how to work on it, and
[How Nifty works](https://istorry.github.io/nifty/internals/architecture.html) follows a
program from source text to a window on screen.

## License

Nifty is licensed under either of

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)

at your option.

Unless you explicitly state otherwise, any contribution you intentionally submit for
inclusion in Nifty, as defined in the Apache-2.0 license, shall be dual licensed as
above, without any additional terms or conditions.
