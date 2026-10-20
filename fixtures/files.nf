state photo = ""
state attachments: [string] = []
state folder = ""
state export_path = ""

view Choice(title: string, chosen: string) {
  column(spacing: 2) {
    label(title, style: .caption, color: .secondary)
    if chosen.is_empty() {
      label("Nothing chosen", color: .tertiary)
    } else {
      label(chosen, selectable: true)
    }
  }
}

window("Files", width: 520, height: 420) {
  column(spacing: 14, padding: 20) {
    label("Files", style: .title)
    row(spacing: 12, align: .center) {
      button("Choose photo…") { pick_file($photo, title: "Choose a photo", types: ["png", "jpg"]) }
      Choice(title: "Photo", chosen: photo)
    }
    row(spacing: 12, align: .center) {
      button("Attach files…") { pick_files($attachments, title: "Attach files") }
      label("{attachments.len()} attached")
    }
    row(spacing: 12, align: .center) {
      button("Choose folder…") { pick_folder($folder, from: "~") }
      Choice(title: "Folder", chosen: folder)
    }
    row(spacing: 12, align: .center) {
      button("Export…", style: .primary) { pick_save_path($export_path, name: "export.csv", types: ["csv"]) }
      Choice(title: "Export to", chosen: export_path)
    }
  }
}
