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

A project is a folder with a `main.nf` in it. Every `.nf` file in the folder and its
subfolders is part of the app:

```
todo/
  main.nf
  ui/
    header.nf
```

- **One namespace.** The files share their top-level names, so a `view`, `fn`, `struct`
  or `state` declared in one file can be used in all of them. There is no include or
  import between files, and their order doesn't matter, just as declarations within a
  file can come in any order.
- **A name declared twice is an error**, even in two different files, and the error
  points at both declarations.
- **`main.nf` marks the project** and must exist. Windows can be declared in any file.
- A subfolder with its own `main.nf` is a separate project, not part of this one. That
  includes the apps `nf build` writes into `build/`.
- A single `.nf` file given to a command, such as `nf run demo.nf`, is an app on its own,
  even when it sits inside a project folder.

| Command | Does |
|---|---|
| `nf create <name>` | creates `<name>/main.nf` from the starter app |
| `nf check [project]` | reports syntax and type errors without running anything |
| `nf lsp` | runs the language server for editors, over stdin and stdout |
| `nf run [project]` | checks the app, then runs it with native windows |
| `nf dev [project]` | runs the app and reloads it in place on every save; state keeps its value while its name and type stay the same |
| `nf build [project] [--macos \| --windows \| --linux]` | checks the app, then packages it as a native app in `build/` for the system you build on; Linux isn't supported yet |

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
- Strings: `"..."` with escapes `\n \t \" \\ \{ \}`. `{expr}` inside a string interpolates
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
- `$place` creates a **two-way binding** for controls that edit a value, and for
  built-in functions that report into state: `text_field($draft)`,
  `checkbox(checked: $todo.done)`, `download(url, to: folder, progress: $percent,
  status: $status)`. The place must be state, or a `for` variable looping over state,
  optionally followed by fields and list items.
- Values are copied on assignment: after `b = a`, changing `b` leaves `a` alone. A `for`
  variable looping over a place is the exception: it stands for each element, so
  `for todo in todos { todo.done = true }` changes `todos`.
- Lists: `[1, 2, 3]`, `items[0]`, `items.len()`, `items.push(x)`, `items.remove(i)`.
- Built-in functions: `download(url, to:, progress:, status:)` saves a file from the web
  on a background thread, and `pick_file`, `pick_files`, `pick_folder` and
  `pick_save_path` ask the user for a path with the system's file dialog; see
  [Built-in functions](functions.md). Their names are reserved like element names.

## UI

A UI block is a sequence of elements. `if` and `for` work inside it to show elements
conditionally or once per list item. Element results are not assigned to variables; a
view is a description, and the compiler builds and updates the native widgets.

### Elements in v0

| Element | Arguments | Trailing block |
|---|---|---|
| `window(title, width:, height:)` | size defaults to 480 × 320 | children |
| `column(spacing:, padding:, align:, justify:, surface:, width:, max_width:, grow:)` | all optional | children |
| `row(spacing:, padding:, align:, justify:, surface:, width:, max_width:, grow:)` | all optional | children |
| `label(text, style:, color:, align:, selectable:)` | `style`: `.body` (default), `.large_title`, `.title`, `.headline`, `.caption`; see [`label`](elements.md#label) | — |
| `button(title, style:, icon:)` | `style`: `.normal` (default), `.primary`, `.plain`, `.icon`; see [`button`](elements.md#button) | action |
| `text_field($value, placeholder:)` | | — |
| `checkbox(checked: $value, title:)` | `title` optional | — |
| `nav_item(title, value:, selection: $value, icon:, badge:)` | see [`nav_item`](elements.md#nav_item) | — |
| `spacer()` | takes the remaining space | — |
| `divider()` | | — |

Text, titles and bound values can come from any expression and stay up to date. Layout
numbers, styles, flags, the window title and placeholders are written literally, as in
`spacing: 12`, `style: .title`, `selectable: true` and `window("Todos")`.

`align` (cross axis): `.stretch` (default), `.start`, `.center`, `.end`.
`justify` (main axis): `.start` (default), `.center`, `.end`, `.space_between`.
`surface`: `.none` (default), `.sidebar`, `.content`, `.card`; see [`column`](elements.md#column).

Text styles map to each platform's system text styles, so apps follow the system font
and the user's text size (Dynamic Type on macOS, Accessibility text size on Windows)
without extra work. Apps follow dark mode on both platforms, and high contrast on Windows.

### How the UI stays up to date

The compiler records which state each dynamic part of the UI reads, including state read
inside functions it calls. When an action changes state, only the parts that read it are
updated: a label gets its new text, and an `if`, `for` or view whose inputs changed is
rebuilt in place. Showing the UI may read state but never change it; changes happen in
button actions and through bindings.

## Not in v0

Per-view local state, view parameters that are bindings, keyed `for` loops that keep
unchanged rows, enums and `match`, closures as values,
async work, networking, files, custom drawing. Each is planned; none is needed for the
first apps. The syntax of the language features is already settled in
[Syntax decisions](#syntax-decisions), rows 5–11.

## Syntax decisions

Rows 1–4 were settled by comparing this spec with an alternative sketch, and the draft's
form was kept in each case. Rows 5–11 settle the syntax of planned features before they
are built. The reasoning is recorded so it isn't relitigated.

| # | Question | Chosen | Alternative | Why |
|---|---|---|---|---|
| 1 | Parentheses on elements | `label("Hi")` | `label "Hi"` | Keep parentheses as the one form. A paren-less shorthand reads well for one string, but every element with options needs parentheses anyway, which gives two ways to write the same thing. The same holds for views and empty argument lists: `view Header() { … }`, `row() { … }`. |
| 2 | Where element settings go | Arguments: `window("Counter", width: 300, height: 200) { … }` | Lines in the block: `title: "Counter"`, `size: { width: 300  height: 200 }` | Arguments. The block then holds only children, and Nifty needs no object-literal syntax. |
| 3 | Enum values | `align: .center` | `align: center` | `.center` for now. A bare `center` can collide with a variable named `center`. Once the type checker exists, bare names could work when nothing else has that name. |
| 4 | Named windows | `window("Counter")` | `window main { … }` | Add names when multiple windows arrive, so code can open or close a window by name: `window main("Counter") { … }`. |
| 5 | Apps bigger than one file | Every `.nf` file in the project folder is part of the app | `$include "ui/header.nf"`, or `import` between your own files | Top-level declarations already work in any order, so splitting them across files changes nothing. A project is one namespace, and a name declared twice is an error that points at both. `$` already means a binding, and a textual include can paste a file twice or depend on order. `import` is kept for packages from outside the project. |
| 6 | View parameters that edit a value | `view Stepper(value: $int) { … }`, used as `Stepper(value: $count)` | `inout int`, `binding int` | `$` in the type matches `$` at the call site. Inside the view, `value` is read and assigned like any value, and `$value` passes the binding on. |
| 7 | State that belongs to one view | `state` at the start of a view's body | A separate `local` keyword | The same word for the same idea. Each view instance has its own copy, identified by its place in the UI and, inside a keyed `for`, by its key. |
| 8 | Keyed `for` | `for todo in todos by todo.id { … }` | `for todo in todos, key: todo.id`, or keying any struct with an `id` field automatically | One short keyword, and keys are always written down. A `for` without `by` keeps rebuilding rows as it does today. |
| 9 | Enums | `enum Load { idle, loading(percent: int), failed(message: string) }`, values written `.idle` or `Load.idle` | `PascalCase` cases | `snake_case` cases match the built-in `.center` and `.space_between`. Built-in options become ordinary enums, so `.name` works the same way for both. |
| 10 | `match` | `.loading(percent) => label("{percent}%")`, or `=> { … }` for several lines | `.loading(percent) { … }`, like `if` | `=>` reads well when `match` is an expression (`let text = match load { … }`) as well as a statement or a UI block. A `match` must cover every case. |
| 11 | Functions as values | Type `fn(int) -> bool`, value `{ todo in !todo.done }` | `fn(todo) { … }`, `\|todo\| …` | A trailing block is already a function with no arguments; naming its arguments before `in` extends the same form. The main use is views that take callbacks, such as `view Row(on_delete: fn())`. |
