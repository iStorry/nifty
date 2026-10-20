# What's next for Nifty

Nifty is at **version 0**: the smallest language that can build a polished counter and
todo app. Everything in it is meant to stay; later versions add to it.

## Working today

- The full v0 language: state, functions, structs, lists, views and windows.
- Nine UI elements, with flexbox layout, and label colors, alignment and selectable
  text.
- `download`, which saves a file from the web in the background and reports its
  progress into state.
- `nf create`, `nf check`, `nf run`, and `nf dev`, which reloads the app on every save
  and keeps its state.
- `nf build`, which packages your app as a `.app` you can share.
- Native apps on **macOS**.
- Editor support in VS Code and Zed through `nf lsp`.

## Planned

**Tooling and platforms**

- A Windows (Win32) backend, in progress in `crates/nifty-windows`, then Linux (GTK).
  `nf build --windows` and `--linux` will work once they land.
- `nf build` compiling apps to machine code, instead of bundling the program with a
  runner that interprets it.

**Language**

- State that belongs to one view, instead of the whole app.
- Passing bindings into views, so your own views can edit values.
- Keyed `for` loops that keep unchanged rows instead of rebuilding them.
- Enums and `match`.
- Functions as values (closures).
- Modules and imports, for apps bigger than one file.

**Doing more**

- More background work, networking and files, building on `download`: other requests,
  reading and writing files, cancelling.
- Custom drawing.

## Have an idea?

Open an [issue](https://github.com/istorry/nifty/issues). The
[language specification](reference/language.md#syntax-decisions) records why the syntax
looks the way it does, so check there first if you're proposing a change to it.
