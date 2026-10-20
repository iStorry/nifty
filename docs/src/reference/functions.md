# Built-in functions

Functions that come with Nifty. Call them from your own functions and from button
actions, like any function you write. They can't be called while showing the UI, since
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
| when it's saved | `100` | `Saved to /Users/you/Downloads/report.pdf` |
| when it fails | unchanged | `Failed: ` and the reason, such as `http status: 404` |

`progress` only moves while downloading when the server says how large the file is;
otherwise it jumps to `100` once the file is saved.

**Where the file goes:** the file is named after the last part of the address, so
`…/files/report.pdf` saves as `report.pdf`, and `…/my%20file.pdf` as `my file.pdf`.
Characters a file name can't hold, such as `/` or `:`, become `_`. If a file with that name is already in the
folder, the download is saved next to it as `report (1).pdf`, then `report (2).pdf`,
and so on; it never replaces a file. A folder starting with `~` is inside your home
folder. If a download fails, the partly saved file is removed.

**Several at once:** each call starts its own download, so you can bind each one to an
item in a list. The [downloader example](../examples/downloader.md) does this:

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

> **Where this comes from:** the list of built-in functions is defined once in
> [`crates/nifty-check/src/functions.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-check/src/functions.rs).
> The checker and the editor's autocomplete both read it, so they always agree.
