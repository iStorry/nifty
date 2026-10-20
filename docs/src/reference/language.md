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
- `window(...)` declares a window. An app has one or more. `window settings(...)` names
  one, for `open_window(settings)` and `close_window(settings)`.
- `menu(...)` declares a menu of the app's menu bar; see [Menus](#ui).

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
  `pick_save_path` ask the user for a path with the system's file dialog, and
  `open_window` and `close_window` open and close a named window; see
  [Built-in functions](functions.md). Their names are reserved like element names.

## UI

A UI block is a sequence of elements. `if` and `for` work inside it to show elements
conditionally or once per list item. Element results are not assigned to variables; a
view is a description, and the compiler builds and updates the native widgets.

### Elements in v0

| Element | Arguments | Trailing block |
|---|---|---|
| `window name(title, width:, height:, backdrop:, open:)` | `name` optional, for code to open and close it by; size defaults to 480 × 320; `backdrop`: `.standard` (default), `.translucent`, `.solid`; `open: false` waits for code to open it | children |
| `column(spacing:, padding:, align:, justify:, surface:, width:, max_width:, grow:)` | all optional | children |
| `row(spacing:, padding:, align:, justify:, surface:, width:, max_width:, grow:)` | all optional | children |
| `scroll(spacing:, padding:, align:, justify:, surface:, width:, max_width:)` | a `column` that takes the space left and scrolls its children; see [`scroll`](elements.md#scroll) | children |
| `label(text, style:, color:, align:, selectable:)` | `style`: `.body` (default), `.large_title`, `.title`, `.headline`, `.caption`; see [`label`](elements.md#label) | — |
| `button(title, style:, icon:, selected:)` | `style`: `.normal` (default), `.primary`, `.plain`, `.icon`; `selected`, a `bool` that stays up to date; see [`button`](elements.md#button) | action |
| `text_field($value, placeholder:, lines:)` | `lines`, a whole number from 1 (default) to 100, makes it that many lines tall | action on **Return**, one-line fields only; optional |
| `checkbox(checked: $value, title:)` | `title` optional | — |
| `switch(on: $value, title:)` | `title` optional; see [`switch`](elements.md#switch) | — |
| `slider($value, min:, max:)` | a bound `int`; `min` and `max` written as whole numbers, `0` and `100` by default | — |
| `picker($value, options:)` | a bound `string`, and a `[string]` of options; see [`picker`](elements.md#picker) | — |
| `nav_item(title, value:, selection: $value, icon:, badge:)` | see [`nav_item`](elements.md#nav_item) | — |
| `image(path, width:, height:)` | `path`, a full path or one starting with `~`; `width` and `height` written as numbers, keeping the image's shape when only one is given; see [`image`](elements.md#image) | — |
| `progress(value:)` | `value`, from 0 to 100, optional; without it the bar shows that work is under way | — |
| `spacer()` | takes the remaining space | — |
| `menu(title)` | at the top level, a menu of the menu bar; in a menu, a submenu; see [`menu`](elements.md#menu) | entries |
| `menu_item(title, shortcut:)` | in a menu; `shortcut` such as `"n"` or `"shift+n"`, with Cmd on macOS and Ctrl on Windows | action |
| `context_menu()` | among a stack's children; shown when the stack is right-clicked; see [`context_menu`](elements.md#context_menu) | entries |
| `drop_target($dropped)` | among a stack's children; a bound `[string]` that a drop of files and links fills with their paths and addresses; see [`drop_target`](elements.md#drop_target) | action after a drop; optional |
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
[Syntax decisions](#syntax-decisions), rows 5–11, and
[What Nifty supports](../roadmap.md) tracks where each one stands.

## Syntax decisions

Rows 1–4 were settled by comparing this spec with an alternative sketch, and the draft's
form was kept in each case. Rows 5–11 settle the syntax of planned features before they
are built, and rows from 12 on were settled as the UI elements they describe were built.
The reasoning is recorded so it isn't relitigated.

| # | Question | Chosen | Alternative | Why |
|---|---|---|---|---|
| 1 | Parentheses on elements | `label("Hi")` | `label "Hi"` | Keep parentheses as the one form. A paren-less shorthand reads well for one string, but every element with options needs parentheses anyway, which gives two ways to write the same thing. The same holds for views and empty argument lists: `view Header() { … }`, `row() { … }`. |
| 2 | Where element settings go | Arguments: `window("Counter", width: 300, height: 200) { … }` | Lines in the block: `title: "Counter"`, `size: { width: 300  height: 200 }` | Arguments. The block then holds only children, and Nifty needs no object-literal syntax. |
| 3 | Enum values | `align: .center` | `align: center` | `.center` for now. A bare `center` can collide with a variable named `center`. Once the type checker exists, bare names could work when nothing else has that name. |
| 4 | Named windows | `window("Counter")`, and `window main("Counter") { … }` for one code opens or closes | `window main { … }` | Names came with opening and closing windows from code: `open_window(main)`. A window without a name is still `window("Counter")`, since only windows code refers to need one. |
| 5 | Apps bigger than one file | Every `.nf` file in the project folder is part of the app | `$include "ui/header.nf"`, or `import` between your own files | Top-level declarations already work in any order, so splitting them across files changes nothing. A project is one namespace, and a name declared twice is an error that points at both. `$` already means a binding, and a textual include can paste a file twice or depend on order. `import` is kept for packages from outside the project. |
| 6 | View parameters that edit a value | `view Stepper(value: $int) { … }`, used as `Stepper(value: $count)` | `inout int`, `binding int` | `$` in the type matches `$` at the call site. Inside the view, `value` is read and assigned like any value, and `$value` passes the binding on. |
| 7 | State that belongs to one view | `state` at the start of a view's body | A separate `local` keyword | The same word for the same idea. Each view instance has its own copy, identified by its place in the UI and, inside a keyed `for`, by its key. |
| 8 | Keyed `for` | `for todo in todos by todo.id { … }` | `for todo in todos, key: todo.id`, or keying any struct with an `id` field automatically | One short keyword, and keys are always written down. A `for` without `by` keeps rebuilding rows as it does today. |
| 9 | Enums | `enum Load { idle, loading(percent: int), failed(message: string) }`, values written `.idle` or `Load.idle` | `PascalCase` cases | `snake_case` cases match the built-in `.center` and `.space_between`. Built-in options become ordinary enums, so `.name` works the same way for both. |
| 10 | `match` | `.loading(percent) => label("{percent}%")`, or `=> { … }` for several lines | `.loading(percent) { … }`, like `if` | `=>` reads well when `match` is an expression (`let text = match load { … }`) as well as a statement or a UI block. A `match` must cover every case. |
| 11 | Functions as values | Type `fn(int) -> bool`, value `{ todo in !todo.done }` | `fn(todo) { … }`, `\|todo\| …` | A trailing block is already a function with no arguments; naming its arguments before `in` extends the same form. The main use is views that take callbacks, such as `view Row(on_delete: fn())`. |
| 12 | A switch's binding | `switch(on: $wifi, title: "Wi-Fi")` | `checkbox(checked: $wifi, style: .switch)` | A switch is its own control on both platforms, `NSSwitch` and Fluent's toggle switch, laid out differently from a checkbox, so it is its own element. `on:` says what the switch's state means, where `checked:` describes a tick. |
| 13 | A slider's range | `slider($volume, min: 0, max: 10)`, whole numbers written literally | A range expression, or `float` values | Ranges are layout, written literally like sizes, and the same on both platforms. Whole numbers cover counts, limits and percentages; a `float` slider can come later as its own setting without changing this one. |
| 14 | A picker's options | `picker($speed, options: speeds)`, any `[string]` expression | Options written as a block of elements | Options are data that can come from state and change as the app runs, so they are an expression, and the binding holds the picked option itself rather than its position. |
| 15 | Picking one of a few choices | A selected state for buttons: `button("All", selected: filter == "all") { filter = "all" }` | A segmented control: `segments($filter, options: ["All", "Done"])` | Both platforms have toggle buttons: macOS's on-off buttons, and Fluent's toggle buttons, which Win32 builds as push-like checkboxes that screen readers announce as pressed. Win32 has no segmented control, so one would be a single control Nifty draws whose segments screen readers can't reach. Buttons also keep their icons, titles and styles, and work in a `for`. |
| 16 | Several lines of text | `text_field($notes, lines: 4)` | A separate `text_area($notes)` element | It edits the same bound text the same way, so it is the same element; how many lines tall it is says how big it is, as `width` does for a stack, and one line stays the default. |
| 17 | Where an image comes from | `image(path, width:, height:)`, a full path, or one from `~` | `image(named: "logo")` for images that come with the app | A path is text, so an image can show a file the app just downloaded or the user picked. Images that come with an app need `nf build` to bundle them and a rule for finding them next to the code, which is left for later, so a relative path written directly is an error rather than a guess. |
| 18 | A text field's action | A trailing block: `text_field($query) { search() }`, run on Return | `on_submit: { … }`, or `action:` | A trailing block is already how a control takes its action, so a text field takes one the same way; leaving it out keeps Return for the window's primary button. |
| 19 | Menus and shortcuts | Top-level `menu("File") { menu_item("New", shortcut: "n") { … } }`, with `divider()` and nested `menu`s | Menus as a setting of `window`, or shortcuts written `"cmd+n"` | Menus are the app's, not one window's, so they sit at the top level like windows, and their items take an action the way buttons do. A shortcut always uses the platform's command key, so one shortcut means Cmd+N on macOS and Ctrl+N on Windows; writing `cmd` or `ctrl` would make code platform-specific. |
| 20 | Context menus | A `context_menu() { … }` child of the stack it belongs to, holding the same `menu_item`s, `divider()`s and submenus as a `menu` | A `context_menu:` setting on each element, or a block on `menu` with a `context: true` flag | A child can be added inside any stack, a view's or a `for` row's, without every element growing a setting, and it reuses the items menus already have; its items run with the names around them, so a row's menu acts on its row. Context menu items take no shortcut: a shortcut can't say which row it means. |
| 21 | Opening and closing windows | `open_window(settings)` and `close_window(settings)`, with `open: false` on a window that waits for code | Methods on the name, `settings.open()`, or a `bool` state bound to each window | Built-in functions are how code asks the system for things, as `pick_file` does, and a window's name is not a value with methods. A bound `bool` would have to say which window is open when the user closes one, which a request doesn't. |
| 22 | Dropping files and links | A `drop_target($dropped) { … }` child of the stack it belongs to, writing a `[string]` of paths and addresses, then running its block | A `drop:` setting on stacks, or a callback that receives the list as a parameter | It covers its stack the way `context_menu` does, and reports into state through a binding, as `pick_files($paths)` does, so code reads what was dropped like any state and the block only decides what to do with it. Files and links share one list of strings, since a link and a path are both where something is. |
