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

view TodoRow(todo: Todo) {
  row(spacing: 8, align: .center) {
    checkbox(checked: $todo.done)
    label(todo.title)
  }
}

window("Todos", width: 360, height: 480) {
  column(spacing: 12, padding: 20) {
    label("Todos", style: .title)
    row(spacing: 8) {
      text_field($draft, placeholder: "What needs doing?")
      button("Add", style: .primary) { add_todo() }
    }
    for todo in todos {
      TodoRow(todo: todo)
    }
    label("{todos.len()} items", style: .caption)
  }
}
