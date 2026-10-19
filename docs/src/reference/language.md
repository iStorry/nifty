# The Nifty language — v0

Nifty is a small, statically typed language for native desktop apps. A `.nf` file
describes state, logic and UI; `nf build` turns it into a native binary that uses the
operating system's own widgets (AppKit on macOS, Win32 on Windows, GTK on Linux).

v0 is the smallest language that can express a polished counter and todo app. Everything
here is meant to stay; later versions add to it.

## A complete app

```nf
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
```

## Projects and the `nf` tool

A project is a folder whose entry point is `main.nf`:

```
todo/
  main.nf
```

| Command | Does |
|---|---|
| `nf create <name>` | creates `<name>/main.nf` from the starter app |
| `nf check [project]` | reports syntax and type errors without running anything |
| `nf lsp` | runs the language server for editors, over stdin and stdout |
| `nf run [project]` | checks the app, then runs it with native windows |
| `nf dev [project]` | runs the app and reloads it in place on every save; state keeps its value while its name and type stay the same |
| `nf build [project] [--macos \| --windows \| --linux]` | checks the app, then packages it as a native app in `build/`; macOS only so far, and the default is the system you build on |

`project` is a project folder or a single `.nf` file, and defaults to the current
folder, so inside a project `nf run` and `nf build` need no arguments.

## Lexical structure

- Source is UTF-8. Comments run from `//` to the end of the line.
- **Newlines end statements.** There are no semicolons. Inside `( )` and `[ ]` newlines
  are ignored, and a line ending in a binary operator continues on the next line.
- Identifiers: `[A-Za-z_][A-Za-z0-9_]*`. By convention values and functions use
  `snake_case`, and types and views use `PascalCase`.
- Keywords: `state let fn view struct if else for in return true false`.
- Integers: `42`, `1_000`. Floats: `3.14`. A float needs digits on both sides of the `.`.
- Strings: `"..."` with escapes `\n \t \" \\ \{`. `{expr}` inside a string interpolates
  any expression: `"{count} of {total}"`.

## Types

| Type | Values |
|---|---|
| `int` | 64-bit signed integer |
| `float` | 64-bit float |
| `bool` | `true`, `false` |
| `string` | immutable UTF-8 text |
| `[T]` | growable list of `T` |
| `Name` | a declared `struct` |

Types are inferred from initial values; write them only where nothing can be inferred,
such as an empty list (`state todos: [Todo] = []`), parameters and return types.

There is no `null`. There are no implicit conversions; `int` and `float` do not mix.

## Declarations

Top-level declarations may appear in any order.

```nf
state count = 0                       // app state; the UI updates when it changes
struct Point { x: float, y: float }   // or one field per line
fn area(w: float, h: float) -> float { return w * h }
view Badge(text: string) { label(text, style: .caption) }
window("App", width: 400, height: 300) { ... }
```

- `state` declares app-wide mutable state. Every view that reads a state value updates
  automatically when it is assigned. The compiler computes which widgets depend on which
  state, so an update touches only those widgets: no diffing, no virtual DOM.
- `fn` declares a function. Its return type defaults to "nothing".
- `view` declares a reusable piece of UI with parameters, used like an element. Its
  parameters are values: a view shows what it is given and is rebuilt when that changes.
- `struct` declares a record type, constructed with named arguments: `Todo(title: "x", done: false)`.
- `window(...)` declares a window. An app has one or more.

## Statements

```nf
let total = a + b           // a local variable
count = 0                   // assignment to state, a field or a list element
count += 1                  // also -=, *=, /=
if x > 0 { ... } else if x < 0 { ... } else { ... }
for item in items { ... }
return value
do_something()              // a call as a statement
```

## Expressions

From loosest to tightest binding:

| Operators | |
|---|---|
| `\|\|` | logical or |
| `&&` | logical and |
| `==  !=  <  <=  >  >=` | comparison (non-associative) |
| `+  -` | add, subtract; `+` also joins strings |
| `*  /  %` | multiply, divide, remainder |
| `!  -  $` | not, negate, two-way binding |
| `f(x)  a.b  a[i]` | call, field or method, index |

- Calls take positional arguments first, then named ones: `button("Save", style: .primary)`.
- A call may be followed by a **trailing block** on the same line: `button("+") { count += 1 }`.
  For layout elements the block holds children; for controls it is the action.
- `.name` is an **implicit member**: the value comes from the expected type, so
  `align: .center` means the `center` alignment.
- `$place` creates a **two-way binding** for controls that edit a value:
  `text_field($draft)`, `checkbox(checked: $todo.done)`. The place must be state, or a
  `for` variable looping over state, optionally followed by fields and list items.
- Values are copied on assignment: after `b = a`, changing `b` leaves `a` alone. A `for`
  variable looping over a place is the exception: it stands for each element, so
  `for todo in todos { todo.done = true }` changes `todos`.
- Lists: `[1, 2, 3]`, `items[0]`, `items.len()`, `items.push(x)`, `items.remove(i)`.

## UI

A UI block is a sequence of elements. `if` and `for` work inside it to show elements
conditionally or once per list item. Element results are not assigned to variables; a
view is a description, and the compiler builds and updates the native widgets.

### Elements in v0

| Element | Arguments | Trailing block |
|---|---|---|
| `window(title, width:, height:)` | size defaults to 480 × 320 | children |
| `column(spacing:, padding:, align:, justify:)` | all optional | children |
| `row(spacing:, padding:, align:, justify:)` | all optional | children |
| `label(text, style:)` | `style`: `.body` (default), `.title`, `.headline`, `.caption` | — |
| `button(title, style:)` | `style`: `.normal` (default), `.primary` | action |
| `text_field($value, placeholder:)` | | — |
| `checkbox(checked: $value, title:)` | `title` optional | — |
| `spacer()` | takes the remaining space | — |
| `divider()` | | — |

Text, titles and bound values can come from any expression and stay up to date. Layout
numbers, styles, the window title and placeholders are written literally, as in
`spacing: 12`, `style: .title` and `window("Todos")`.

`align` (cross axis): `.stretch` (default), `.start`, `.center`, `.end`.
`justify` (main axis): `.start` (default), `.center`, `.end`, `.space_between`.

Text styles map to each platform's system text styles, so apps follow the system font,
Dynamic Type and dark mode without extra work.

### How the UI stays up to date

The compiler records which state each dynamic part of the UI reads, including state read
inside functions it calls. When an action changes state, only the parts that read it are
updated: a label gets its new text, and an `if`, `for` or view whose inputs changed is
rebuilt in place. Showing the UI may read state but never change it; changes happen in
button actions and through bindings.

## Not in v0

Per-view local state, view parameters that are bindings, keyed `for` loops that keep
unchanged rows, enums and `match`, closures as values, modules and imports,
async work, networking, files, custom drawing. Each is planned; none is needed for the
first apps.

## Syntax decisions

Settled by comparing this spec with an alternative sketch. The draft's form was kept in
each case; the reasoning is recorded so it isn't relitigated.

| # | Question | Chosen | Alternative | Why |
|---|---|---|---|---|
| 1 | Parentheses on elements | `label("Hi")` | `label "Hi"` | Keep parentheses as the one form. A paren-less shorthand reads well for one string, but every element with options needs parentheses anyway, which gives two ways to write the same thing. |
| 2 | Where element settings go | Arguments: `window("Counter", width: 300, height: 200) { … }` | Lines in the block: `title: "Counter"`, `size: { width: 300  height: 200 }` | Arguments. The block then holds only children, and Nifty needs no object-literal syntax. |
| 3 | Enum values | `align: .center` | `align: center` | `.center` for now. A bare `center` can collide with a variable named `center`. Once the type checker exists, bare names could work when nothing else has that name. |
| 4 | Named windows | `window("Counter")` | `window main { … }` | Add names when multiple windows arrive, so code can open or close a window by name: `window main("Counter") { … }`. |
