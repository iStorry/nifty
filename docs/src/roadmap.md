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
- Loops that name each item's position, `for index, item in items`, and loops that
  count, `for i in 0..n`.
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
- Switches, sliders for whole numbers, and pickers that choose a string from a list.
- A selected state for buttons, for picking one of a few choices.
- Text fields several lines tall, which wrap and scroll.
- Images from files, at their own size or a fixed width or height.
- A text field's own action, run on **Return**.
- Menus and keyboard shortcuts: the menu bar on macOS, and every window's menu bar on
  Windows.
- Context menus, shown when the user right-clicks a stack.
- Opening and closing windows from code, with windows that start closed.
- Dropping files and links onto a stack, which writes their paths and addresses into
  state.
- Window backdrops: a translucent window on macOS, and Mica or Acrylic in the frame on
  Windows 11.
- `download`, which saves a file from the web in the background and reports its
  progress into state: a percentage, a status, the bytes received, the file's size and
  the speed, which `format_bytes` shows as text such as `1.5 MB`. Each download has an
  id, which keeps its reports with its list item and which `pause_download`,
  `resume_download` and `cancel_download` take; resuming continues with an HTTP range
  request, or starts over when the server can't send part of a file. A download can
  fetch its file over several connections at once, be saved under a name the app
  chooses, send request headers, and retry failed connections after a wait.
- File dialogs: `pick_file`, `pick_files`, `pick_folder` and `pick_save_path` ask the
  user for a path with the system's own Open and Save dialogs.
- `nf create`, `nf check`, `nf run`, and `nf dev`, which reloads the app on every save
  and keeps its state.
- `nf build`, which compiles your app to machine code and packages it to share.
- Editor support in VS Code and Zed through `nf lsp`.

## In progress

- Download control: several connections per file, choosing the file name, and request
  headers and retries ([#18](https://github.com/istorry/nifty/pull/18)).

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

- Images that come with an app, which `nf build` bundles and the app finds next to its
  code. Images read from full paths, and paths from `~`, work today.
- Custom drawing.

**Doing more**

- More background work, networking and files, building on `download`: other requests,
  and reading and writing files.

## Have an idea?

Open an [issue](https://github.com/istorry/nifty/issues). The
[language specification](reference/language.md#syntax-decisions) records why the syntax
looks the way it does, so check there first if you're proposing a change to it.
