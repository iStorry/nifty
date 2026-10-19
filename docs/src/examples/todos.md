# Todos

A todo list: type a task, add it, tick it off, and see how many are left.

![The todos app](../images/todos.svg)

```nf
{{#include ../../../examples/todos/main.nf}}
```

**Run it:**

```bash
nf run examples/todos
```

**Source:** [`examples/todos/main.nf`](https://github.com/istorry/nifty/blob/main/examples/todos/main.nf)

## How it works

- **`struct Todo`** describes one task: a `title` and whether it's `done`.
- **`state todos: [Todo] = []`** is the list of tasks, starting empty.
- **`state draft = ""`** is the text in the input field.
- **`add_todo()`** adds the draft as a new task, then clears the field.
- **`remaining()`** counts the tasks not yet done. Because the header calls it, the
  header updates whenever `todos` changes.
- **`view Header`** is a reusable piece of UI that takes the count as a parameter.
- **`text_field($draft, …)`** and **`checkbox(checked: $todo.done, …)`** use `$` to
  edit state directly. See [Two-way bindings](../guide/bindings.md).

Want to build it yourself, step by step? Follow the [todo tutorial](../tutorials/todos.md).
