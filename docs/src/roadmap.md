# What's next for Nifty

Nifty is at **version 0**: the smallest language that can build a polished counter and
todo app. Everything in it is meant to stay; later versions add to it.

## Working today

- The full v0 language: state, functions, structs, lists, views and windows.
- Apps split across files: every `.nf` file in the project folder is part of the app,
  in the CLI, `nf dev` reloads and the editor.
- Built-in UI elements, with flexbox layout, and label colors, alignment and selectable
  text.
- `download`, which saves a file from the web in the background and reports its
  progress into state.
- `nf create`, `nf check`, `nf run`, and `nf dev`, which reloads the app on every save
  and keeps its state.
- `nf build`, which compiles your app to machine code and packages it to share: a `.app`
  on macOS, or a folder with an `.exe` on Windows.
- Native apps on **macOS** (AppKit) and **Windows** (Win32).
- Editor support in VS Code and Zed through `nf lsp`.

## Planned

**Tooling and platforms**

- A Linux (GTK) backend, so `nf build --linux` works too.

**Language**

- State that belongs to one view, instead of the whole app.
- Passing bindings into views, so your own views can edit values.
- Keyed `for` loops that keep unchanged rows instead of rebuilding them.
- Enums and `match`.
- Functions as values (closures).

**Doing more**

- More background work, networking and files, building on `download`: other requests,
  reading and writing files, cancelling.
- Custom drawing.

## Have an idea?

Open an [issue](https://github.com/istorry/nifty/issues). The
[language specification](reference/language.md#syntax-decisions) records why the syntax
looks the way it does, so check there first if you're proposing a change to it.
