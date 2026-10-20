struct Download {
  url: string
  percent: int
  status: string
}

state url = ""
state folder = "~/Downloads"
state downloads: [Download] = []

fn start_download() {
  if url != "" {
    downloads.push(Download(url: url, percent: 0, status: ""))
    let last = downloads.len() - 1
    download(url, to: folder, progress: $downloads[last].percent, status: $downloads[last].status)
    url = ""
  }
}

view DownloadRow(item: Download) {
  column(spacing: 2) {
    row(spacing: 8, align: .center) {
      label(item.url, selectable: true)
      spacer()
      label("{item.percent}%", style: .caption)
    }
    progress(value: item.percent)
    label(item.status, style: .caption)
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
      if downloads.is_empty() {
        label("Nothing downloaded yet", color: .tertiary, align: .center)
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
