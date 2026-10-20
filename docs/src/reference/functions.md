# Built-in functions

Functions that come with Nifty: [`download`](#download), the
[file dialogs](#file-dialogs-pick_file-pick_files-pick_folder-pick_save_path), and
[`open_window` and `close_window`](#windows-open_window-close_window). Call them
from your own functions and from button actions, like any function you write. They can't be called while showing the UI, since
they change state.

---

## `download`

Saves the file at a web address into a folder. It runs in the background, so the app
stays responsive, and reports how it's going by writing to two pieces of state.

```nf
state percent = 0
state status = ""

button("Download") {
  download("https://example.com/report.pdf", to: "~/Downloads", progress: $percent, status: $status)
}
label("{percent}% {status}")
```

| Setting | Kind | |
|---|---|---|
| *(first)* url | `string` | *required*; an `http://` or `https://` address |
| `to` | `string` | *required*; the folder to save into, created if missing |
| `progress` | binding to an `int` | *required*; the percentage done, `0` to `100` |
| `status` | binding to a `string` | *required*; where the download stands, in words |

**What the bindings show:**

| When | `progress` | `status` |
|---|---|---|
| right after the call | `0` | `Downloading…` |
| as data arrives | `0` to `100` | unchanged |
| when it's saved | `100` | `Saved to /Users/you/Downloads/report.pdf`, or `Saved to C:\Users\you\Downloads\report.pdf` on Windows |
| when it fails | unchanged | `Failed: ` and the reason, such as `http status: 404` |

`progress` only moves while downloading when the server says how large the file is;
otherwise it jumps to `100` once the file is saved.

**Where the file goes:** the file is named after the last part of the address, so
`…/files/report.pdf` saves as `report.pdf`, and `…/my%20file.pdf` as `my file.pdf`.
Characters a file name can't hold, such as `/` or `:`, become `_`. If a file with that name is already in the
folder, the download is saved next to it as `report (1).pdf`, then `report (2).pdf`,
and so on; it never replaces a file. A name Windows keeps for devices, such as `con` or
`nul.txt`, gets an `_` in front, on every system, so the file opens anywhere. A folder
starting with `~` is inside your home folder. If a download fails, the partly saved file
is removed.

**Windows folders:** `\` starts an escape in a Nifty string, so write each backslash in a
path twice, as in `to: "C:\\Users\\you\\Downloads"`, or use `/`, which Windows accepts
too: `to: "C:/Users/you/Downloads"`. `"~/Downloads"` works on every system.

**Several at once:** each call starts its own download, so you can bind each one to an
item in a list:

```nf
downloads.push(Download(url: url, percent: 0, status: ""))
let last = downloads.len() - 1
download(url, to: folder, progress: $downloads[last].percent, status: $downloads[last].status)
```

The bindings point at a position in the list. Removing an item further up while a
download is still running makes its reports land on the item that moved into its place,
or fail with an error once that position no longer exists.

> **With `nf dev`:** saving your code restarts the app's program, which stops listening to
> downloads still running. Their state keeps its last value, and each one stops, removing
> its partial file, the next time it has progress to report.

---

## File dialogs: `pick_file`, `pick_files`, `pick_folder`, `pick_save_path`

Ask the user to choose a file or folder with the system's own dialog, and write what
they chose into state. The dialog opens once the button's action has finished, so the
rest of the action runs first and the window is up to date behind it.

```nf
state photo = ""

button("Choose photo…") { pick_file($photo, title: "Choose a photo", types: ["png", "jpg"]) }
label(photo)
```

| Function | Asks for | Binding |
|---|---|---|
| `pick_file` | one existing file | a `string` |
| `pick_files` | one or more existing files | a `[string]` |
| `pick_folder` | one existing folder | a `string` |
| `pick_save_path` | where to save a new file; it may already exist | a `string` |

| Setting | Kind | |
|---|---|---|
| *(first)* | binding | *required*; where the chosen path goes |
| `title` | `string` | text shown in the dialog |
| `types` | `[string]` | the file extensions allowed, such as `["png", "jpg"]`, with or without the dot; any file when left out. Not on `pick_folder` |
| `name` | `string` | `pick_save_path` only: the file name it suggests |
| `from` | `string` | the folder it opens in; `~` is your home folder |

**What the binding gets:** the full path, such as `/Users/you/Pictures/cat.png`, or
`C:\Users\you\Pictures\cat.png` on Windows. `pick_files` replaces the whole list. If
the user cancels, the binding keeps its value, so test it to see whether anything was
chosen. `pick_save_path` only chooses the path; it doesn't create or write the file, and
when the user picks a file that exists, the system has already asked them to confirm
replacing it.

**On each system:**

| | macOS | Windows |
|---|---|---|
| Looks like | a sheet sliding down from the active window, or its own window when none is active | the system's Open or Save As dialog, in front of the active window, which waits until it closes |
| `title` | shown as a message at the top of the panel | the dialog's title bar |
| `types` | other files are greyed out; saving adds the first type's extension | a filter listing the types; saving adds the first type's extension |
| `from` | ignored when the folder doesn't exist | ignored when the folder doesn't exist |

If the system can't show the dialog, or an extension in `types` is one macOS doesn't
know, the app reports an error pointing at the call, and the binding keeps its value.

> **With `nf dev`:** saving your code while a dialog is open restarts the program behind
> it; whatever the user then chooses is dropped.

A whole app that uses all four:

```nf
{{#include ../../../fixtures/files.nf}}
```

---

## Windows: `open_window`, `close_window`

Open and close one of the app's windows, which code names by the name written between
`window` and its title.

```nf
window settings("Settings", open: false) {
  button("Done") { close_window(settings) }
}

menu("Downloader") {
  menu_item("Settings…", shortcut: ",") { open_window(settings) }
}
```

| Function | | |
|---|---|---|
| `open_window(name)` | *(first)* a window's name | opens the window, building its content from the state as it is now; when it is open already, brings it to the front |
| `close_window(name)` | *(first)* a window's name | closes the window, if it's open |

Both happen once the code that called them finishes, and the last call for a window
decides: closing a window and opening it again in one action leaves it open. Windows
open before others close, so an action can swap one window for another.

The app quits when its last window closes, whether the code or the user closes it, so
closing the only open window ends the app. A window that's closed and opened again is a
new window: a text field's selection or a scroll position starts over. With `nf dev`,
windows open when you save stay open, and closed ones stay closed.

| | macOS | Windows |
|---|---|---|
| New window | centered on the screen, in front, with the keyboard | centered on its monitor, in front, with the keyboard |
| Bringing an open window to the front | it comes to the front with the keyboard | it comes to the front; if another app has the foreground, Windows may only flash its taskbar button |

---

> **Where this comes from:** the list of built-in functions is defined once in
> [`crates/nifty-check/src/functions.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-check/src/functions.rs).
> The checker and the editor's autocomplete both read it, so they always agree.
