# Functions

A **function** gives a name to some code so you can use it from several places, or
just to keep your window tidy.

```nf
fn add_todo() {
  if draft != "" {
    todos.push(Todo(title: draft, done: false))
    draft = ""
  }
}
```

Call it by name with parentheses:

```nf
button("Add") { add_todo() }
```

## Parameters

Inputs go in the parentheses, each with a name and a type:

```nf
fn greet(name: string, times: int) {
  ...
}
```

## Returning a value

If a function gives back an answer, write its type after `->`, and use `return`:

```nf
fn area(width: float, height: float) -> float {
  return width * height
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
```

Nifty checks every path through the function returns something. If one path can reach
the end without a `return`, you'll get:

```text
Error: `remaining` can reach its end without returning a `int`
```

A function without `->` returns nothing. You can still use a bare `return` to leave it
early.

## Functions in the UI

Functions that return a value can be used anywhere the UI needs one:

```nf
label("{remaining()} left")
Header(left: remaining())
```

Nifty looks inside the function, sees it reads `todos`, and updates that label whenever
`todos` changes. You don't have to tell it.

Functions used in the UI may only **read** state, not change it. Functions that change
state belong in button actions.

## Control flow

These work inside functions and button actions:

```nf
if count > 10 {
  ...
} else if count > 5 {
  ...
} else {
  ...
}

for todo in todos {
  ...
}
```

The condition of an `if` must be a `bool`. There's no "truthy": write
`if name != ""` or `if !name.is_empty()`, not `if name`.

## Order doesn't matter

Top-level things (`state`, `fn`, `view`, `struct`, `window`) can go in any order. A
function can use a struct declared below it, and a window can call a function declared
at the end of the file.
