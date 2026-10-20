state section = "inbox"
state unread = 3

window("Mail", width: 720, height: 460) {
  row() {
    column(surface: .sidebar, width: 200, padding: 10) {
      label("Mailboxes", style: .caption)
      nav_item("Inbox", value: "inbox", selection: $section, icon: .inbox, badge: "{unread}")
      nav_item("Sent", value: "sent", selection: $section, icon: .send)
      nav_item("Archive", value: "archive", selection: $section, icon: .archive)
    }
    column(surface: .content, grow: true, padding: 24, align: .center) {
      column(max_width: 480, spacing: 12) {
        row(align: .center) {
          label("Mail", style: .large_title)
          spacer()
          button("Mark all read", style: .icon, icon: .check) { unread = 0 }
        }
        column(surface: .card, padding: 16, spacing: 4) {
          label("Welcome to {section}", style: .headline)
          label("Cards group related content on the page.", color: .secondary)
        }
      }
    }
  }
}
