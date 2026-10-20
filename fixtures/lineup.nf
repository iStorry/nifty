state names = ["Ada", "Grace", "Linus"]
state draft = ""
state rating = 3

fn position_of(name: string) -> int {
  for index, item in names {
    if item == name {
      return index
    }
  }
  return -1
}

fn add_name() {
  if draft != "" && position_of(draft) < 0 {
    names.push(draft)
    draft = ""
  }
}

fn meter(filled: int) -> string {
  let bar = ""
  for step in 0..5 {
    if step < filled {
      bar = bar + "●"
    } else {
      bar = bar + "○"
    }
  }
  return bar
}

window("Lineup", width: 320, height: 420) {
  column(spacing: 12, padding: 20) {
    for index, name in names {
      row(spacing: 8) {
        label("{index + 1}. {name}")
        spacer()
        button("Remove {name}") { names.remove(index) }
      }
    }
    row(spacing: 8) {
      text_field($draft, placeholder: "Name")
      button("Add") { add_name() }
    }
    if position_of(draft) >= 0 {
      label("{draft} is already number {position_of(draft) + 1}", style: .caption)
    }
    divider()
    row(spacing: 4) {
      button("Fewer") { rating -= 1 }
      for star in 0..rating {
        label("★{star + 1}")
      }
      button("More") { rating += 1 }
    }
    label(meter(rating))
  }
}
