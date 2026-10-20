# Building the UI

In Nifty you **describe** what the screen should look like. You don't create widgets,
keep track of them, or update them. Nifty does that.

## Windows

Every app has at least one `window`. Windows go at the top level of the file:

```nf
window("Notes", width: 400, height: 300) {
  label("Hello")
}
```

The title is required. `width` and `height` are optional; the default size is 480 × 320.

An app can have more than one window. Each is its own `window(…) { … }` block.

## Elements

Inside a window you put **elements**: built-in pieces of UI like `label`, `button` and
`column`. They all look like a call:

```nf
label("Hello", style: .title)
```

- Unnamed arguments come first (`"Hello"`), then named ones (`style: .title`).
- Parentheses are always needed, even when empty: `spacer()`, `column() { … }`.

Many elements take a **block** in `{ }` right after the closing parenthesis, on the
same line. What the block means depends on the element:

| Element | Its block holds |
|---|---|
| `window`, `column`, `row`, `scroll` | the **children** to show inside it |
| `button` | the **action**: code that runs when it's clicked |

```nf
column() {                          // children
  button("Save") { save() }         // action
}
```

The elements are `window`, `column`, `row`, `scroll`, `label`, `button`, `text_field`,
`checkbox`, `nav_item`, `progress`, `spacer` and `divider`. The [UI elements reference](../reference/elements.md) lists every
one with all its settings.

## Showing things conditionally

Use `if` inside the UI to show elements only sometimes:

```nf
if todos.is_empty() {
  label("Nothing to do!")
} else {
  label("{remaining()} left")
}
```

## Showing a list

Use `for` to show elements once per item:

```nf
for todo in todos {
  checkbox(checked: $todo.done, title: todo.title)
}
```

## Only UI goes in the UI

A window or view body contains elements, views, `if` and `for`. It can't contain
statements like `let` or `count = 0`. Those go in functions or button actions:

```text
Error: only elements, `if` and `for` can appear here; put logic in a function or a button's action
```

## Views: your own elements

When a part of the UI gets big, or you use it more than once, give it a name with
`view`:

```nf
view Header(left: int) {
  row(align: .center) {
    label("Todos", style: .title)
    spacer()
    label("{left} left", style: .caption)
  }
}
```

Then use it like any element, giving its parameters by name:

```nf
window("Todos") {
  column() {
    Header(left: remaining())
  }
}
```

By convention view names start with a capital letter (`Header`), while elements,
functions and values use `snake_case` (`text_field`, `add_todo`).

A view shows what it's given. When `remaining()` changes, the `Header` is rebuilt with
the new value.

## Literal vs. live values

Some settings must be written directly in the code, because they shape the window
rather than show data:

- layout numbers like `spacing: 12` and `width: 320`,
- options like `style: .title` and `align: .center`,
- a window's title, and a text field's placeholder.

Text that's *shown*, like a label's text, a button's title or a checkbox's title, can be
any expression, and stays up to date.

```nf
label("{count}")              // ✓ live text
window(title_of_app()) { }    // ✗ the window title must be written out: window("Todos")
```
