state section = "Inbox"

view SectionButton(name: string) {
  if section == name {
    button("● {name}") { section = name }
  } else {
    button(name) { section = name }
  }
}

window("Mail", width: 720, height: 460) {
  row() {
    column(surface: .sidebar, width: 180, padding: 16, spacing: 6, align: .start) {
      label("Mailboxes", style: .caption)
      SectionButton(name: "Inbox")
      SectionButton(name: "Sent")
      SectionButton(name: "Archive")
    }
    column(surface: .content, grow: true, padding: 24, align: .center) {
      column(max_width: 480, spacing: 12) {
        label(section, style: .title)
        column(surface: .card, padding: 16, spacing: 4) {
          label("Welcome to {section}", style: .headline)
          label("Cards group related content on the page.", color: .secondary)
        }
      }
    }
  }
}
