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

window("Downloader", width: 480, height: 420) {
  column(spacing: 12, padding: 20) {
    label("Downloader", style: .title)
    row(spacing: 8) {
      text_field($url, placeholder: "https://example.com/file.zip")
      button("Download", style: .primary) { start_download() }
    }
    row(spacing: 8, align: .center) {
      label("Save to", style: .caption)
      text_field($folder, placeholder: "~/Downloads")
    }
    divider()
    if downloads.is_empty() {
      label("Nothing downloaded yet", color: .tertiary, align: .center)
    }
    for item in downloads {
      DownloadRow(item: item)
    }
  }
}
