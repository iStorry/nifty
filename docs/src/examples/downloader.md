# Downloader

Paste a web address, press **Download**, and watch it save into a folder. Each download
gets its own row with its progress and where it ended up.

```nf
{{#include ../../../examples/downloader/main.nf}}
```

**Run it:**

```bash
nf run examples/downloader
```

**Source:** [`examples/downloader/main.nf`](https://github.com/istorry/nifty/blob/main/examples/downloader/main.nf)

## How it works

- **`struct Download`** is one row: the address, how far along it is, and its status.
- **`state downloads: [Download] = []`** holds every download started so far.
- **`start_download()`** adds a row, then calls
  [`download`](../reference/functions.md#download) with bindings to that row's
  `percent` and `status`. The download runs in the background; the button returns
  straight away, so you can start another one while it runs.
- As data arrives, `download` writes the new percentage into the row, and the
  **`DownloadRow`** view showing it updates, just as if a button had changed it.
- **`text_field($folder, …)`** lets you pick where files go. It starts at
  `~/Downloads`, your Downloads folder.
