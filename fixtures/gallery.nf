state name = ""
state subscribed = true
state dark_icons = false
state clicks = 0
state place = "home"
state notify = true
state volume = 4
state speed = "Fast"
state filter = "all"
state notes = "Several lines\nof notes"
state photo = ""

view Section(title: string) {
  label(title, style: .headline, color: .accent)
}

window("Gallery", width: 760, height: 760, backdrop: .standard) {
  scroll(spacing: 16, padding: 20) {
    row(align: .center) {
      label("Nifty gallery", style: .title)
      spacer()
      label("every element and style", style: .caption)
    }
    divider()

    row(spacing: 24) {
      column(spacing: 8) {
        Section(title: "Text styles")
        label("Large title", style: .large_title)
        label("Title", style: .title)
        label("Headline", style: .headline)
        label("Body", style: .body)
        label("Caption", style: .caption)

        Section(title: "Text colors")
        row(spacing: 12) {
          label("primary", color: .primary)
          label("secondary", color: .secondary)
          label("tertiary", color: .tertiary)
        }
        row(spacing: 12) {
          label("accent", color: .accent)
          label("success", color: .success)
          label("warning", color: .warning)
          label("danger", color: .danger)
        }

        Section(title: "Text alignment")
        label("start", align: .start)
        label("center", align: .center)
        label("end", align: .end)

        Section(title: "Selectable text")
        label("Select and copy me", selectable: true)
      }

      column(spacing: 8) {
        Section(title: "Buttons")
        row(spacing: 8) {
          button("Normal", style: .normal) { clicks += 1 }
          button("Primary", style: .primary) { clicks += 1 }
          button("Reset") { clicks = 0 }
        }
        row(spacing: 8, align: .center) {
          button("Add", icon: .plus) { clicks += 1 }
          button("Plain", style: .plain, icon: .star) { clicks += 1 }
          button("Delete", style: .icon, icon: .trash) { clicks += 1 }
        }
        label("Clicked {clicks} times", style: .caption)
        row(spacing: 4, align: .center) {
          button("All", selected: filter == "all") { filter = "all" }
          button("Active", style: .plain, selected: filter == "active") { filter = "active" }
          button("Starred", style: .icon, icon: .star, selected: filter == "starred") { filter = "starred" }
          label("Showing {filter}", style: .caption)
        }

        Section(title: "Text field")
        text_field($name, placeholder: "Your name")
        text_field($notes, lines: 3)
        if name == "" {
          label("Type something above", color: .tertiary)
        } else {
          label("Hello, {name}!", color: .success)
        }

        Section(title: "Image")
        row(spacing: 8, align: .center) {
          button("Choose image…") { pick_file($photo, types: ["png", "jpg"]) }
          image(photo, width: 64, height: 64)
        }

        Section(title: "Progress")
        progress(value: clicks * 10)
        progress()

        Section(title: "Switch, slider and picker")
        switch(on: $notify, title: "Notify me")
        row(spacing: 8, align: .center) {
          label("Volume {volume}")
          slider($volume, min: 0, max: 10)
        }
        picker($speed, options: ["Fast", "Balanced", "Slow"])

        Section(title: "Checkboxes")
        checkbox(checked: $subscribed, title: "Subscribe to updates")
        checkbox(checked: $dark_icons, title: "Use dark icons")
        checkbox(checked: $subscribed)
        if subscribed {
          label("You're subscribed", color: .success)
        } else {
          label("You're not subscribed", color: .warning)
        }
      }
    }

    divider()
    Section(title: "Icons")
    row(spacing: 4) {
      button("plus", style: .icon, icon: .plus) { clicks += 1 }
      button("trash", style: .icon, icon: .trash) { clicks += 1 }
      button("star", style: .icon, icon: .star) { clicks += 1 }
      button("star_fill", style: .icon, icon: .star_fill) { clicks += 1 }
      button("check", style: .icon, icon: .check) { clicks += 1 }
      button("check_circle", style: .icon, icon: .check_circle) { clicks += 1 }
      button("circle", style: .icon, icon: .circle) { clicks += 1 }
      button("home", style: .icon, icon: .home) { clicks += 1 }
      button("inbox", style: .icon, icon: .inbox) { clicks += 1 }
      button("archive", style: .icon, icon: .archive) { clicks += 1 }
    }
    row(spacing: 4) {
      button("list", style: .icon, icon: .list) { clicks += 1 }
      button("calendar", style: .icon, icon: .calendar) { clicks += 1 }
      button("sun", style: .icon, icon: .sun) { clicks += 1 }
      button("flag", style: .icon, icon: .flag) { clicks += 1 }
      button("close", style: .icon, icon: .close) { clicks += 1 }
      button("gear", style: .icon, icon: .gear) { clicks += 1 }
      button("search", style: .icon, icon: .search) { clicks += 1 }
      button("mail", style: .icon, icon: .mail) { clicks += 1 }
      button("send", style: .icon, icon: .send) { clicks += 1 }
      button("folder", style: .icon, icon: .folder) { clicks += 1 }
    }
    row(spacing: 4) {
      button("clock", style: .icon, icon: .clock) { clicks += 1 }
      button("tag", style: .icon, icon: .tag) { clicks += 1 }
      button("person", style: .icon, icon: .person) { clicks += 1 }
      button("document", style: .icon, icon: .document) { clicks += 1 }
      button("download", style: .icon, icon: .download) { clicks += 1 }
      button("heart", style: .icon, icon: .heart) { clicks += 1 }
      button("bell", style: .icon, icon: .bell) { clicks += 1 }
      button("pencil", style: .icon, icon: .pencil) { clicks += 1 }
      button("share", style: .icon, icon: .share) { clicks += 1 }
      button("info", style: .icon, icon: .info) { clicks += 1 }
    }

    divider()
    Section(title: "Nav items")
    row(spacing: 12) {
      column(surface: .sidebar, width: 200, padding: 8) {
        nav_item("Home", value: "home", selection: $place, icon: .home)
        nav_item("Inbox", value: "inbox", selection: $place, icon: .inbox, badge: "{clicks}")
        nav_item("No icon", value: "plain", selection: $place)
      }
      label("Showing {place}")
    }

    divider()
    Section(title: "Surfaces")
    row(spacing: 12) {
      column(surface: .sidebar, width: 140, padding: 12) {
        label("surface: .sidebar", style: .caption)
      }
      column(surface: .content, grow: true, padding: 12, spacing: 8) {
        label("surface: .content, grow: true", style: .caption)
        column(surface: .card, max_width: 260, padding: 12) {
          label("surface: .card, max_width: 260", style: .caption)
        }
        column(surface: .none) {
          label("surface: .none", style: .caption)
        }
      }
    }

    divider()
    Section(title: "Layout")
    column(align: .stretch) {
      button("align: .stretch") { clicks += 1 }
    }
    column(align: .start) {
      button("align: .start") { clicks += 1 }
    }
    column(align: .center) {
      button("align: .center") { clicks += 1 }
    }
    column(align: .end) {
      button("align: .end") { clicks += 1 }
    }
    row(spacing: 8, justify: .start) {
      label(".start", style: .caption)
      label("A")
      label("B")
    }
    row(spacing: 8, justify: .center) {
      label(".center", style: .caption)
      label("A")
      label("B")
    }
    row(spacing: 8, justify: .end) {
      label(".end", style: .caption)
      label("A")
      label("B")
    }
    row(spacing: 8, justify: .space_between, padding: 4) {
      label(".space_between", style: .caption)
      label("A")
      label("B")
    }
  }
}

window("Translucent backdrop", width: 280, height: 120, backdrop: .translucent) {
  column(padding: 20, align: .center, justify: .center) {
    label("backdrop: .translucent", style: .headline)
  }
}

window("Solid backdrop", width: 280, height: 120, backdrop: .solid) {
  column(padding: 20, align: .center, justify: .center) {
    label("backdrop: .solid", style: .headline)
  }
}
