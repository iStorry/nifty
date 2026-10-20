# Cheat sheet

All of Nifty on one page. Each heading links to the full explanation.

## [Declarations](../guide/state.md)

```nf
state count = 0                          // app state; the UI follows it
state todos: [Todo] = []                 // empty lists need a type

struct Todo {                            // a record type
  title: string
  done: bool
}

fn add(a: int, b: int) -> int {          // a function
  return a + b
}

view Badge(text: string) {               // your own element
  label(text, style: .caption)
}

window("App", width: 400, height: 300) { // a window
  Badge(text: "hi")
}
```

## [Types](../guide/values.md)

`int` · `float` · `bool` · `string` · `[T]` (a list) · struct names

## [Statements](../guide/functions.md)

```nf
let total = a + b          // local variable
count = 0                  // assign
count += 1                 // also -=  *=  /=
if x > 0 { } else if x < 0 { } else { }
for item in items { }
return value
add_todo()                 // a call
```

## [Expressions](../guide/values.md#operators)

```nf
"Hi {name}, you have {count * 2} points"   // interpolation
a || b    a && b    !a                     // logic
==  !=  <  <=  >  >=                       // comparison
+  -  *  /  %                              // math (+ also joins strings)
items[0]    todo.title                     // index, field
Todo(title: "x", done: false)              // make a struct
[1, 2, 3]                                  // a list
.center                                    // an option
$draft                                     // a two-way binding
```

## [Lists and strings](../guide/structs-and-lists.md)

```nf
items.len()      items.is_empty()
items.push(x)    items.remove(i)
text.len()       text.is_empty()
```

## [UI elements](elements.md)

```nf
window("Title", width: 480, height: 320) { … }   // top level only
window("HUD", backdrop: .translucent) { … }       // .standard .translucent .solid
column(spacing: 12, padding: 20, align: .center, justify: .start) { … }
row(spacing: 8, align: .center) { … }
column(surface: .sidebar, width: 200) { … }   // .none .sidebar .content .card
column(surface: .content, grow: true) { … }    // grow fills the space left
column(max_width: 600) { … }                    // fills up to 600 points wide
label("Text", style: .title)              // .body .large_title .title .headline .caption
label("Oops", color: .danger, align: .center, selectable: true)
button("Save", style: .primary) { … }     // .normal .primary .plain .icon
button("Delete", style: .icon, icon: .trash) { … }
nav_item("Inbox", value: "inbox", selection: $section, icon: .inbox, badge: "3")
text_field($draft, placeholder: "Type…")
progress(value: percent)                  // 0 to 100; progress() when you can't say
checkbox(checked: $done, title: "Done")
spacer()
divider()
```

`align`: `.stretch` `.start` `.center` `.end` (on `label`: `.start` `.center` `.end`)

`color`: `.primary` `.secondary` `.tertiary` `.accent` `.success` `.warning` `.danger`

`justify`: `.start` `.center` `.end` `.space_between`

## [Built-in functions](functions.md)

```nf
download(url, to: "~/Downloads", progress: $percent, status: $status)
pick_file($path, title: "Open", types: ["png", "jpg"], from: "~/Pictures")
pick_files($paths)
pick_folder($folder)
pick_save_path($path, name: "export.csv", types: ["csv"])
```

## [UI logic](../guide/ui.md)

```nf
if todos.is_empty() {
  label("Nothing to do")
}
for todo in todos {
  checkbox(checked: $todo.done, title: todo.title)
}
```

## [The `nf` command](cli.md)

```bash
nf create my-app     # new project
nf run               # check, then run
nf dev               # run, and reload on every save
nf check             # check only
nf build             # make an app in build/ (.app on macOS, .exe folder on Windows)
```

Every `.nf` file in the project folder is part of the app, with no imports: a view
declared in `ui/header.nf` can be used in `main.nf`.

## Rules of thumb

- One statement per line. No semicolons.
- Elements always have parentheses: `column() { }`, `spacer()`.
- Turn numbers into text with `"{n}"`, not `+`.
- `int` and `float` don't mix.
- Only button actions, bindings, `download` and the `pick_…` dialogs change state.
