struct Todo {
  title: string
  done: bool
}

state todos: [Todo] = []
state draft = ""

fn add_todo() {
  if draft != "" {
    todos.push(Todo(title: draft, done: false))
    draft = ""
  }
}

fn remaining() -> int {
  let count = 0
  for todo in todos {
    if !todo.done {
      count += 1
    }
  }
  return count
}

view Header(left: int) {
  row(align: .center) {
    label("Todos", style: .title)
    spacer()
    label("{left} left", style: .caption)
  }
}

window("Todos", width: 360, height: 480) {
  column(spacing: 12, padding: 20) {
    Header(left: remaining())
    row(spacing: 8) {
      text_field($draft, placeholder: "What needs doing?")
      button("Add", style: .primary) { add_todo() }
    }
    divider()
    for todo in todos {
      checkbox(checked: $todo.done, title: todo.title)
    }
  }
}
