# Two-way bindings

Most of the UI only **shows** values. A label shows text; it can't change it. But
some controls let people **edit** a value: a text field you type into, a checkbox you
click.

For those, you hand over the value with a `$` in front. That's a **two-way binding**:

```nf
state draft = ""
state agreed = false

window("Form") {
  column(spacing: 8, padding: 16) {
    text_field($draft, placeholder: "Your name")
    checkbox(checked: $agreed, title: "I agree")
    label("Hello, {draft}!")
  }
}
```

The two ways are:

1. **State → control:** the field shows `draft`. If code changes `draft` (say, a button
   sets `draft = ""`), the field changes too.
2. **Control → state:** when someone types, `draft` changes. The label above updates as
   they type.

## What can be bound

| Control | Binds to |
|---|---|
| `text_field($value)` | a `string` |
| `checkbox(checked: $value)` | a `bool` |
| `download(…, progress: $value, status: $text)` | an `int` and a `string` |

`text_field` and `checkbox` are the only elements that take a binding. The built-in
function [`download`](../reference/functions.md#download) takes two as well, and writes
to them as the download goes.

The thing after `$` must be somewhere a value is stored, starting from state:

```nf
$draft                 // ✓ a state
$todo.done             // ✓ a field of a `for` item looping over state
$settings.dark_mode    // ✓ a field of a state struct
$todos[0].done         // ✓ a field of a list item in state
$remaining()           // ✗ a function's result isn't stored anywhere
```

## Binding inside a loop

Inside `for todo in todos`, `todo` stands for the real item in the list, so binding
to it edits the list in state:

```nf
for todo in todos {
  checkbox(checked: $todo.done, title: todo.title)
}
```

Clicking the third checkbox changes `todos[2].done`.

## Forgetting the `$`

If you leave out the `$`, Nifty tells you:

```text
Error: `text_field` needs a two-way binding to a `string`, such as `$name`
```
