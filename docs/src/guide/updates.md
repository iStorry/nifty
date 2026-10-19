# How the UI stays up to date

You've seen that in Nifty you never redraw anything by hand. This page explains what's
happening, so the behavior never surprises you.

## The short version

1. Before your app runs, Nifty finds out **which state each part of the UI reads**.
2. When state changes, Nifty updates **only the parts that read it**.

That's all. No diffing a whole tree, and no virtual DOM.

## An example

```nf
state count = 0
state name = "Ada"

window("Demo") {
  column() {
    label("Hi, {name}")              // reads name
    label("Count: {count}")          // reads count
    if count > 5 {                   // reads count
      label("Big number!")
    }
    button("+") { count += 1 }
  }
}
```

When you click **+**:

- `count` changes.
- The second label reads `count`, so it gets its new text.
- The `if` reads `count`, so Nifty checks it again and shows or hides "Big number!".
- The first label only reads `name`, so it isn't touched at all.

## It looks inside functions

If the UI calls a function, Nifty follows it, and anything it reads counts too:

```nf
fn remaining() -> int {
  let count = 0
  for todo in todos {
    if !todo.done {
      count += 1
    }
  }
  return count
}

window("Todos") {
  label("{remaining()} left")   // updates when todos changes
}
```

## What gets updated, and how

| Part of the UI | When what it reads changes… |
|---|---|
| Text (label text, button title, …) | the native widget gets its new text |
| A bound control (`text_field`, `checkbox`) | it shows the new value |
| `if` | it's checked again; its contents are rebuilt in place if needed |
| `for` | its rows are rebuilt in place |
| A view | it's rebuilt in place with its new parameters |

Everything else on screen stays exactly as it was, including what's typed in other text
fields and where the cursor is.

## When changes happen

Changes only happen **in response to something**: a button click, or typing into a
bound control. The changes in one click all happen together, and then the screen
catches up once.

Showing the UI is never allowed to change state (see [State](state.md)). That's what
makes this work: drawing is just reading, so Nifty can redraw any part at any time and
always get the same answer.

## If something goes wrong while running

If a button's action hits an error, like dividing by zero or reading a list item that
isn't there, the app stops and `nf run` shows the line that caused it. Nifty stops
rather than carrying on, because the action was interrupted halfway and the app's state
might no longer make sense.
