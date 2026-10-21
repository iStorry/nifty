struct Download {
  id: string
  url: string
  percent: int
  status: string
  received: int
  total: int
  speed: int
}

state url = ""
state folder = "~/Downloads"
state connections = 4
state downloads: [Download] = []
state dropped: [string] = []

fn start_download() {
  if url != "" {
    downloads.push(Download(id: "", url: url, percent: 0, status: "", received: 0, total: 0, speed: 0))
    let last = downloads.len() - 1
    download(url, to: folder, id: $downloads[last].id, progress: $downloads[last].percent, status: $downloads[last].status, received: $downloads[last].received, total: $downloads[last].total, speed: $downloads[last].speed, connections: connections, retries: 3)
    url = ""
  }
}

fn download_dropped() {
  for link in dropped {
    url = link
    start_download()
  }
}

// Removing an item that is still downloading cancels its download.
fn remove_download(id: string) {
  for index, item in downloads {
    if item.id == id {
      downloads.remove(index)
      return
    }
  }
}

fn is_running(item: Download) -> bool {
  return item.status == "Downloading…" || item.status == "Retrying…"
}

fn is_paused(item: Download) -> bool {
  return item.status == "Paused"
}

fn sizes(item: Download) -> string {
  if item.total == 0 {
    return format_bytes(item.received)
  }
  if item.speed == 0 {
    return "{format_bytes(item.received)} of {format_bytes(item.total)}"
  }
  return "{format_bytes(item.received)} of {format_bytes(item.total)}, {format_bytes(item.speed)}/s"
}

view DownloadRow(item: Download) {
  column(spacing: 2) {
    context_menu() {
      menu_item("Remove") { remove_download(item.id) }
    }
    row(spacing: 8, align: .center) {
      label(item.url, selectable: true)
      spacer()
      label("{item.percent}%", style: .caption)
      if is_running(item) {
        button("Pause") { pause_download(item.id) }
      }
      if is_paused(item) {
        button("Resume") { resume_download(item.id) }
      }
      if is_running(item) || is_paused(item) {
        button("Cancel", style: .icon, icon: .close) { cancel_download(item.id) }
      }
    }
    progress(value: item.percent)
    row(spacing: 8) {
      label(item.status, style: .caption)
      spacer()
      label(sizes(item), style: .caption)
    }
  }
}

window main("Downloader", width: 480, height: 420) {
  column(spacing: 12, padding: 20) {
    row(align: .center) {
      label("Downloader", style: .title)
      spacer()
      button("Settings", style: .icon, icon: .gear) { open_window(settings) }
    }
    row(spacing: 8) {
      text_field($url, placeholder: "https://example.com/file.zip") { start_download() }
      button("Download", style: .primary) { start_download() }
    }
    divider()
    scroll(spacing: 12) {
      context_menu() {
        menu_item("Clear List") { downloads = [] }
      }
      drop_target($dropped) { download_dropped() }
      if downloads.is_empty() {
        label("Nothing downloaded yet; drop links here", color: .tertiary, align: .center)
      }
      for item in downloads {
        DownloadRow(item: item)
      }
    }
  }
}

window settings("Settings", width: 360, height: 160, open: false) {
  column(spacing: 12, padding: 20) {
    row(spacing: 8, align: .center) {
      label("Save to", style: .caption)
      text_field($folder, placeholder: "~/Downloads")
    }
    row(spacing: 8, align: .center) {
      label("Connections", style: .caption)
      slider($connections, min: 1, max: 8)
      label("{connections}", style: .caption)
    }
    row(justify: .end) {
      button("Done", style: .primary) { close_window(settings) }
    }
  }
}

menu("Downloader") {
  menu_item("Settings…", shortcut: ",") { open_window(settings) }
}
