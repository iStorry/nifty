# Built-in functions

Functions that come with Nifty: [`download`](#download), its controls
[`pause_download`, `resume_download` and `cancel_download`](#pause_download-resume_download-cancel_download),
[`format_bytes`](#format_bytes), the
[file dialogs](#file-dialogs-pick_file-pick_files-pick_folder-pick_save_path), and
[`open_window` and `close_window`](#windows-open_window-close_window). Call them
from your own functions and from button actions, like any function you write. All but
`format_bytes` change state, so the UI can't call them while it is shown; `format_bytes`
only gives back text, so labels can show it.

---

## `download`

Saves the file at a web address into a folder. It runs in the background, so the app
stays responsive, and reports how it's going by writing to state: always a percentage and
a status, and the bytes received, the file's size and the speed when you bind them.

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
| `received` | binding to an `int` | the bytes saved so far |
| `total` | binding to an `int` | the file's size in bytes, once the server says; `0` until then |
| `speed` | binding to an `int` | bytes per second, averaged over the last two seconds |
| `id` | binding to a `string` | the download's id, for [pausing, resuming and cancelling](#pause_download-resume_download-cancel_download); needed when the bindings are in a list item |
| `name` | `string` | the name to save the file as, made safe the same way as a name from the address; `""`, the default, names it after the address |
| `connections` | `int` | how many connections fetch the file at once, from `1` (the default) to `16`; see **Several connections** below |

**What the bindings show:**

| When | `progress` | `status` |
|---|---|---|
| When | `progress` | `status` | `received` | `total` | `speed` |
|---|---|---|---|---|---|
| right after the call | `0` | `Downloading…` | `0` | `0` | `0` |
| as data arrives | `0` to `100` | unchanged | bytes so far | the size, if the server said | bytes per second |
| when it's saved | `100` | `Saved to /Users/you/Downloads/report.pdf`, or `Saved to C:\Users\you\Downloads\report.pdf` on Windows | the file's size | the file's size | `0` |
| when it fails | unchanged | `Failed: ` and the reason, such as `http status: 404` | unchanged | unchanged | `0` |
| when it's paused | unchanged | `Paused` | bytes saved | unchanged | `0` |
| when it's resumed | as data arrives | `Downloading…` | as data arrives | as data arrives | as data arrives |
| when it's cancelled | unchanged | `Cancelled` | unchanged | unchanged | `0` |

While data arrives the bindings change at most ten times a second, so a fast download
doesn't keep the app busy redrawing. `progress` and `total` only move while downloading
when the server says how large the file is; otherwise `progress` jumps to `100` and
`total` to the file's size once it is saved. Show sizes and speeds with
[`format_bytes`](#format_bytes):

```nf
label("{format_bytes(received)} of {format_bytes(total)}, {format_bytes(speed)}/s")
```

**Where the file goes:** the file is named after the last part of the address, so
`…/files/report.pdf` saves as `report.pdf`, and `…/my%20file.pdf` as `my file.pdf`.
`name: "Q3 report.pdf"` names it yourself instead, written as it is, without `%`
decoding. Either way, characters a file name can't hold, such as `/` or `:`, become `_`,
so a name such as `../report.pdf` stays in the folder as `.._report.pdf`. If a file with that
name is already in the folder, the download is saved next to it as `report (1).pdf`, then
`report (2).pdf`, and so on; it never replaces a file. A name Windows keeps for devices,
such as `con` or `nul.txt`, gets an `_` in front, on every system, so the file opens
anywhere. A folder starting with `~` is inside your home folder.

While it downloads, the file is called `report.pdf.part`, and it gets its name once it is
saved: the first of `report.pdf`, `report (1).pdf` and so on that is still free then. If a
download fails or is cancelled, the partly saved file is removed.

**Several connections:** `connections: 4` fetches the file over up to four connections
at once, which is faster from a server that limits each connection's speed.

```nf
download(url, to: folder, progress: $percent, status: $status, connections: 4)
```

The first connection asks for the whole file as an HTTP range request. When the server
answers with part of the file and says how large it is, the file is split into nearly
equal parts, no smaller than 64 KB each, so a small file still uses one connection: the
first connection keeps the first part, and a new connection asks for each of the others.
They write into the same `.part` file, each at its own place. When the server can't send
part of a file, or doesn't say how large it is, the first connection fetches all of it
alone. A number below `1` counts as `1`, and one above `16` as `16`.

The bindings report the whole file as before: `received` counts what every connection has
saved, and `speed` adds their speeds together. Pausing stops every connection and keeps
what each part has saved; resuming opens a new connection for each part that isn't
finished, asking for the rest of it. Cancelling stops them all and removes the file.

**Windows folders:** `\` starts an escape in a Nifty string, so write each backslash in a
path twice, as in `to: "C:\\Users\\you\\Downloads"`, or use `/`, which Windows accepts
too: `to: "C:/Users/you/Downloads"`. `"~/Downloads"` works on every system.

**Ids, pausing and cancelling:** bind `id:` to a `string` and the call writes the
download's id there right away, such as `download-3`. Pass it to
[`pause_download`, `resume_download` or `cancel_download`](#pause_download-resume_download-cancel_download)
to control the download. Ids are never reused while the app runs, even across `nf dev`
reloads.

```nf
state id = ""
button("Get") { download(url, to: folder, id: $id, progress: $percent, status: $status) }
button("Pause") { pause_download(id) }
button("Resume") { resume_download(id) }
button("Cancel") { cancel_download(id) }
```

**Several at once:** each call starts its own download, so you can keep each in an item
of a list. A download that reports into a list item needs `id:` in the same item:

```nf
downloads.push(Download(id: "", url: url, percent: 0, status: ""))
let last = downloads.len() - 1
download(url, to: folder, id: $downloads[last].id, progress: $downloads[last].percent, status: $downloads[last].status)
```

The bindings stay with the item that holds the id, wherever it moves: removing items
before it, or rebuilding the list with only some of them, doesn't send its reports to
another item. Before each report, and after each action, the app looks for the item
holding the id and moves the download's bindings in the same item there; a binding
somewhere else, such as a separate state, stays where it is. Removing the item, or
overwriting its id, cancels the download, as `cancel_download` does.

| | macOS | Windows |
|---|---|---|
| Cancelling | the partly saved file is removed at once | the partly saved file is removed at once, unless another program has it open; then `status` says `Failed: cancelled; the partial file … is left behind` |

> **With `nf dev`:** saving your code restarts the app's program, which stops every
> download, running or paused, and removes its partly saved `.part` file before the new
> version starts. Their state keeps its last value, so a `status` still says
> `Downloading…` or `Paused`, and their ids no longer control anything; start them again
> to finish them.
>
> **When the app quits:** on Windows, downloads still running or paused stop and remove
> their `.part` files as the app quits. On macOS the app ends without waiting, so a
> download still running or paused leaves its `.part` file behind.

---

## `pause_download`, `resume_download`, `cancel_download`

Control a running download by the id its `download` call wrote into `id:`.

```nf
for item in downloads {
  button("Pause") { pause_download(item.id) }
  button("Resume") { resume_download(item.id) }
  button("Cancel") { cancel_download(item.id) }
}
```

| Function | | Does |
|---|---|---|
| `pause_download(id)` | *(first)* the id, a `string` | stops downloading for now and keeps the partly saved file; `status` becomes `Paused`, `received` the bytes saved and `speed` `0` |
| `resume_download(id)` | *(first)* the id, a `string` | carries on from where the paused download stopped; `status` goes back to `Downloading…` |
| `cancel_download(id)` | *(first)* the id, a `string` | stops it for good and removes the partly saved file; `status` becomes `Cancelled` and `speed` `0` |

Each happens once the action has finished, in the order the code asked. An id that isn't
running, such as one that has just finished, does nothing, and so does pausing a paused
download or resuming one that isn't paused. Pausing takes effect at once, even while the
server is slow to answer.

**How resuming works:** the download asks the server for the rest of the file, from the
first byte it doesn't have, with an HTTP range request. A server that can't send part of
a file sends all of it instead, and then the download starts over from zero: `received`
and `progress` go back to `0` and climb again, and the file still ends up whole. A paused
download whose file was already complete is saved as soon as it resumes.

| | macOS | Windows |
|---|---|---|
| A paused download's file | `report.pdf.part`, in the folder it is saving to | the same |
| Cancelling | the partly saved file is removed at once | the partly saved file is removed at once, unless another program has it open; then `status` says `Failed: cancelled; the partial file … is left behind` |

---

## `format_bytes`

Gives a number of bytes as text people read easily. It only reads, so labels and other
UI can call it.

```nf
label(format_bytes(1500))       // 1.5 KB
label("{format_bytes(speed)}/s") // such as 2.4 MB/s
```

| Setting | Kind | |
|---|---|---|
| *(first)* bytes | `int` | *required*; the size, in bytes |

Under 1000 bytes it counts them, as `1 byte` or `999 bytes`. From there it uses the
largest of `KB`, `MB`, `GB`, `TB`, `PB` and `EB` that keeps the number at least 1, with one
decimal place, rounded to nearest: `1.5 KB`, `15.2 MB`. Each unit is 1000 of the one
before, as macOS and web browsers count, so a size reads the same on every system;
Windows Explorer counts in 1024s, so it can show the same file slightly smaller. A
negative number keeps its sign.

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
