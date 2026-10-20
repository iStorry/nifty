# Install Nifty

Nifty comes as one command-line tool called **`nf`**. You use it to create, check and
run apps. This page gets it onto your computer. You don't need Rust or anything else
first.

## macOS

Install it with [Homebrew](https://brew.sh):

```bash
brew install istorry/tap/nifty-cli
```

## Windows

Install it with winget, which comes with Windows 10 and 11:

```powershell
winget install nifty-cli
```

Open a **new** terminal window afterwards so it picks up `nf`.

## Check it works

```bash
nf help
```

You should see:

```text
usage: nf <command> [args]

commands:
  create <name>      create a new app in a folder called <name>
  run [project]      run an app (default: the current folder)
  dev [project]      run an app and reload it each time you save, keeping its state
  check [project]    report errors in a project (default: the current folder)
  build [project] [--macos | --windows | --linux]
                     build a native app (default: for this system)
  lsp                run the language server on stdin and stdout, for editors
  version            print the version of nf

A project is a folder with a main.nf file, or a single .nf file.
```

That's it. Next, [make your first app](first-app.md).

To build apps to share with `nf build`, you'll also need your system's linker; see
[Share your app](share.md#what-nf-build-does).

## Updating

| System | Command |
|---|---|
| macOS | `brew upgrade nifty-cli` |
| Windows | `winget upgrade nifty-cli` |

## Building from source

If you'd rather build `nf` yourself, or want to work on Nifty, see
[Building from source](../internals/contributing.md#building-from-source).

## If something goes wrong

| What you see | What to do |
|---|---|
| `command not found: nf` (macOS) | Run `brew doctor` and follow what it says about your `PATH`. |
| `'nf' is not recognized` (Windows) | Open a new terminal window. winget adds `nf` to your `PATH`, but only new windows see it. |
| `No package found matching input criteria` (Windows) | Run `winget source update`, then try again. |
| `libnifty_rt.a … is missing` or `nifty_rt.lib … is missing` | `nf build` needs the runtime library that's installed beside `nf`. Reinstall Nifty with the command above. |
| `can't run the linker` or `xcrun: error` (macOS) | Install Xcode's command line tools: `xcode-select --install`. |
| `building apps needs Microsoft's C++ Build Tools` (Windows) | Install them with the command the error shows, then build again. |
