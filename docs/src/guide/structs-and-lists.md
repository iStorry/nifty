# Structs and lists

## Structs: your own types

A **struct** groups a few values into one thing. Declare it with its fields and their
types:

```nf
struct Todo {
  title: string
  done: bool
}
```

Fields can go one per line (as above) or on one line with commas:

```nf
struct Point { x: float, y: float }
```

Make one by calling the struct's name and giving every field by name:

```nf
let todo = Todo(title: "Buy milk", done: false)
```

Read and change fields with a dot:

```nf
label(todo.title)
todo.done = true
```

## Lists

A list holds any number of values of the same type, in order.

```nf
state numbers = [1, 2, 3]          // a [int]
state names: [string] = []         // an empty list needs its type
state todos: [Todo] = []
```

What you can do with a list:

| Write | Does |
|---|---|
| `items[0]` | the first item (positions start at 0) |
| `items[i] = x` | replace an item |
| `items.len()` | how many items there are |
| `items.is_empty()` | `true` if there are none |
| `items.push(x)` | add `x` to the end |
| `items.remove(i)` | remove the item at position `i` |
| `for item in items { … }` | do something with each item |
| `for index, item in items { … }` | the same, with each item's position |

Asking for an item that isn't there stops the app with a clear message, like
`there is no item 3; the list has 2`.

## Values are copied

When you assign a struct or list, you get a **copy**. Changing the copy leaves the
original alone:

```nf
let a = Todo(title: "x", done: false)
let b = a
b.done = true      // a.done is still false
```

There's one exception, and it's the one you want: a `for` loop over state gives you
the **real** items, so you can change them in place:

```nf
fn finish_all() {
  for todo in todos {
    todo.done = true     // changes the todos in state
  }
}
```

## Lists in the UI

A `for` loop inside a window or view shows its elements once per item:

```nf
for todo in todos {
  checkbox(checked: $todo.done, title: todo.title)
}
```

When `todos` changes, the loop is rebuilt to match.

## Positions and counting

Name a second variable before the item to get its position, counting from 0. A loop
that finds an item can return its position, and a row can remove itself:

```nf
fn position_of(name: string) -> int {
  for index, item in names {
    if item == name {
      return index
    }
  }
  return -1
}
```

To repeat something a number of times, loop over a range. `for i in 0..n` counts `i`
from 0 up to, but not including, `n`, so `0..items.len()` is every position in `items`.
The position and the count are set by the loop, so they can't be assigned.

This app numbers its rows, removes the one you click, and draws a meter with a range:

```nf
{{#include ../../../fixtures/lineup.nf}}
```
