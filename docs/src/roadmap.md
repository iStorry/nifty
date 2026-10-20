# What Nifty supports

Nifty is at **version 0**: the smallest language that can build a polished counter and
todo app. Everything in it is meant to stay; later versions add to it. This page says
what works today, what's being built now, and what isn't there yet.

## Platforms

| | macOS | Windows | Linux |
|---|---|---|---|
| Run apps: `nf run`, `nf dev` | Yes, with AppKit | Yes, with Win32 | Not yet |
| Share apps: `nf build` | A `.app`; needs Xcode's command line tools | A folder with an `.exe`; needs Microsoft's C++ Build Tools | Not yet |
| Install `nf` | Homebrew | winget | Not yet |
| Window backdrops | A translucent window | Mica or Acrylic in the frame, on Windows 11 22H2 and later | Not yet |
| Icons | SF Symbols | Segoe Fluent Icons on Windows 11, Segoe MDL2 Assets on Windows 10 | Not yet |

Where the two platforms draw a setting differently, its entry in
[UI elements](reference/elements.md) or [Built-in functions](reference/functions.md)
says how.

## Supported

- The full v0 language: state, functions, structs, lists, views and windows.
- Apps split across files: every `.nf` file in the project folder is part of the app,
  in the CLI, `nf dev` reloads and the editor.
- Built-in UI elements, with flexbox layout, and label colors, alignment and selectable
  text.
- Surfaces: sidebars, content panes and cards drawn with each platform's own materials,
  fixed and limited widths, and windows that stay wide enough for their content.
- Icons from the system's own sets, borderless and icon-only buttons, a large title
  style, and selectable sidebar rows (`nav_item`).
- Progress bars, filled to a percentage or showing that work is under way.
- Scrolling areas, which scroll their content up and down in the space they're given.
- Window backdrops: a translucent window on macOS, and Mica or Acrylic in the frame on
  Windows 11.
- `download`, which saves a file from the web in the background and reports its
  progress into state.
- File dialogs: `pick_file`, `pick_files`, `pick_folder` and `pick_save_path` ask the
  user for a path with the system's own Open and Save dialogs.
- `nf create`, `nf check`, `nf run`, and `nf dev`, which reloads the app on every save
  and keeps its state.
- `nf build`, which compiles your app to machine code and packages it to share.
- Editor support in VS Code and Zed through `nf lsp`.

## In progress

- Loop indexes and ranges: `for index, item in list` and `for i in 0..n`
  ([#14](https://github.com/istorry/nifty/pull/14)).

## Not supported yet

**Platforms and tooling**

- A Linux (GTK) backend, so `nf build --linux` works too.
- `nf build` on Windows without installing Microsoft's C++ Build Tools first, by
  shipping a linker with `nf`.
- Mica and Acrylic behind the content of Windows windows, not just their frame. The
  system's own controls can't draw on a see-through background, so this waits on Nifty
  drawing its own controls.

**Language**

- State that belongs to one view, instead of the whole app.
- Passing bindings into views, so your own views can edit values.
- Keyed `for` loops that keep unchanged rows instead of rebuilding them.
- Enums and `match`. Until then, a choice such as a filter is a string.
- Constants. Until then, a named value is a function that returns it.
- Functions as values (closures).
- More list and string methods. Lists and strings have `len` and `is_empty`, and lists
  have `push` and `remove`; filtering, counting, sorting and searching are loops for
  now, and wait on closures.

**UI**

- Images.
- Multi-line text fields.
- Pickers, sliders and switches.
- A selected state for buttons, or a segmented control, for picking one of a few
  choices.
- A text field action of its own. Today **Return** clicks the window's `.primary`
  button.
- Menus and keyboard shortcuts.
- Opening and closing windows from code. Every `window` opens when the app starts.
- Custom drawing.

**Doing more**

- More background work, networking and files, building on `download`: other requests,
  reading and writing files, cancelling.

## Have an idea?

Open an [issue](https://github.com/istorry/nifty/issues). The
[language specification](reference/language.md#syntax-decisions) records why the syntax
looks the way it does, so check there first if you're proposing a change to it.
