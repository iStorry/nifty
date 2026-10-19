# Nifty

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

- **Small:** you can learn the whole language in an afternoon.
- **Native:** apps use the system's own widgets, fonts and dark mode.
- **Live:** `nf dev` reloads your app each time you save, keeping its state.
- **Checked:** mistakes are caught before the app runs, and explained in plain words.

Nifty is at version 0.1.0-alpha.1, its first alpha. It runs and builds apps on macOS;
the Windows backend is in progress, and Linux comes after it.

## Get started

Install `nf`, the Nifty tool:

```bash
brew install istorry/tap/nifty-cli
```

Then make an app and run it:

```bash
nf create hello
nf dev hello
```

The [Nifty book](https://istorry.github.io/nifty/) has the
[install guide](https://istorry.github.io/nifty/getting-started/install.html),
tutorials, the language guide and the full reference. Finished apps are in
[`examples/`](examples).

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
