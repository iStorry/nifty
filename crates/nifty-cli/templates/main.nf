state count = 0

window("{{title}}", width: 320, height: 200) {
  column(spacing: 12, align: .center, justify: .center) {
    label("Clicked {count} times", style: .title)
    row(spacing: 8) {
      button("−") { count -= 1 }
      button("+", style: .primary) { count += 1 }
    }
  }
}
