struct Download {
  url: string
  percent: int
  status: string
  received: int
  total: int
  speed: int
}

state url = ""
state folder = "~/Downloads"
state downloads: [Download] = []
state dropped: [string] = []

fn start_download() {
  if url != "" {
    downloads.push(Download(url: url, percent: 0, status: "", received: 0, total: 0, speed: 0))
    let last = downloads.len() - 1
    download(url, to: folder, progress: $downloads[last].percent, status: $downloads[last].status, received: $downloads[last].received, total: $downloads[last].total, speed: $downloads[last].speed)
    url = ""
  }
}

fn download_dropped() {
  for link in dropped {
    url = link
    start_download()
  }
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
    row(spacing: 8, align: .center) {
      label(item.url, selectable: true)
      spacer()
      label("{item.percent}%", style: .caption)
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

window settings("Settings", width: 360, height: 120, open: false) {
  column(spacing: 12, padding: 20) {
    row(spacing: 8, align: .center) {
      label("Save to", style: .caption)
      text_field($folder, placeholder: "~/Downloads")
    }
    row(justify: .end) {
      button("Done", style: .primary) { close_window(settings) }
    }
  }
}

menu("Downloader") {
  menu_item("Settings…", shortcut: ",") { open_window(settings) }
}
