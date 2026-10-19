# State

**State is what your app remembers.** The number in a counter, the todos in a list, the
text someone has typed: all state.

```nf
state count = 0
state draft = ""
state todos: [Todo] = []
```

`state` lines go at the top level of your file, outside any window, function or view.
Every part of the app can read and change them.

## State drives the screen

The special thing about state is that **the UI follows it**. When state changes, every
part of the screen that reads it updates:

```nf
state count = 0

window("Demo") {
  column() {
    label("Count: {count}")         // updates when count changes
    button("+") { count += 1 }
  }
}
```

You never tell a label to redraw. You change the state, and Nifty does the rest. See
[How the UI stays up to date](updates.md) for the details.

## Changing state

You can change state in two places:

1. **In a button's action** (or a function it calls):

   ```nf
   button("Reset") { count = 0 }
   ```

2. **Through a two-way binding**, when someone types in a text field or clicks a
   checkbox:

   ```nf
   text_field($draft)
   ```

The ways to change a value:

```nf
count = 10        // set it
count += 1        // add to it
count -= 1        // take away
count *= 2        // multiply
count /= 2        // divide
todos.push(item)  // add to the end of a list
todos.remove(0)   // remove the first item of a list
todo.done = true  // change one field
todos[0] = item   // replace one list item
```

## Showing state never changes it

Code that *builds the screen* can read state, but it can't change it. This is an
error:

```nf
fn bump() { count += 1 }

window("Oops") {
  label("{bump()}")   // ✗ showing the UI can't change state
}
```

This rule means drawing the screen is always safe and predictable: looking at your
app never changes what it remembers.

## Starting values

A state's starting value can't use other state, because there's no order in which state
gets created:

```nf
state a = 1
state b = a     // ✗ a state's starting value can't use other state
```

If you need `b` to always follow `a`, don't store it. Compute it with a
[function](functions.md) instead.

## Local variables

Inside functions and button actions, `let` makes a temporary variable that disappears
when the block ends:

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
```

Use `state` for things the app should remember, and `let` for working things out.
