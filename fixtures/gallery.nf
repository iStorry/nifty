state name = ""
state subscribed = true
state dark_icons = false
state clicks = 0

view Section(title: string) {
  label(title, style: .headline, color: .accent)
}

window("Gallery", width: 760, height: 760) {
  column(spacing: 16, padding: 20) {
    row(align: .center) {
      label("Nifty gallery", style: .title)
      spacer()
      label("every element and style", style: .caption)
    }
    divider()

    row(spacing: 24) {
      column(spacing: 8) {
        Section(title: "Text styles")
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
        label("Clicked {clicks} times", style: .caption)

        Section(title: "Text field")
        text_field($name, placeholder: "Your name")
        if name == "" {
          label("Type something above", color: .tertiary)
        } else {
          label("Hello, {name}!", color: .success)
        }

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
