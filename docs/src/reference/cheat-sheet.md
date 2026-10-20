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
for index, item in items { } // index: 0, 1, 2, …
for i in 0..n { }          // 0 up to n - 1
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
window settings("Settings", open: false) { … }    // named; opens when code asks
column(spacing: 12, padding: 20, align: .center, justify: .start) { … }
row(spacing: 8, align: .center) { … }
column(surface: .sidebar, width: 200) { … }   // .none .sidebar .content .card
column(surface: .content, grow: true) { … }    // grow fills the space left
column(max_width: 600) { … }                    // fills up to 600 points wide
scroll(spacing: 8) { … }                        // a column in the space left, scrolling
label("Text", style: .title)              // .body .large_title .title .headline .caption
label("Oops", color: .danger, align: .center, selectable: true)
button("Save", style: .primary) { … }     // .normal .primary .plain .icon
button("Delete", style: .icon, icon: .trash) { … }
button("All", selected: filter == "all") { filter = "all" }  // one of a few choices
nav_item("Inbox", value: "inbox", selection: $section, icon: .inbox, badge: "3")
text_field($draft, placeholder: "Type…")
text_field($notes, lines: 4)              // several lines, wrapping and scrolling
text_field($query) { search() }           // Return runs the block
image("~/Pictures/a.png", width: 64)       // keeps its shape; full paths or ~ only
progress(value: percent)                  // 0 to 100; progress() when you can't say
checkbox(checked: $done, title: "Done")
switch(on: $notify, title: "Notify me")
slider($volume, min: 0, max: 10)
picker($speed, options: ["Fast", "Slow"])
spacer()
divider()
```

`align`: `.stretch` `.start` `.center` `.end` (on `label`: `.start` `.center` `.end`)

`color`: `.primary` `.secondary` `.tertiary` `.accent` `.success` `.warning` `.danger`

`justify`: `.start` `.center` `.end` `.space_between`

## [Menus](elements.md#menu)

```nf
menu("File") {                            // top level: a menu of the menu bar
  menu_item("New", shortcut: "n") { … }   // Cmd+N on macOS, Ctrl+N on Windows
  divider()
  menu("Sort By") {                       // a submenu
    menu_item("Name", shortcut: "shift+1") { … }
  }
}

row() {
  context_menu() {                        // shown when the row is right-clicked
    menu_item("Remove") { … }             // no shortcut in a context menu
  }
  drop_target($dropped) { … }             // paths and links dropped on the row
  label(item.name)
}
```

## [Built-in functions](functions.md)

```nf
download(url, to: "~/Downloads", progress: $percent, status: $status)
download(url, to: folder, progress: $p, status: $s, received: $got, total: $size, speed: $rate)
download(url, to: folder, id: $items[last].id, progress: $items[last].percent, status: $items[last].status)
cancel_download(item.id) // removes the partial file; status becomes "Cancelled"
format_bytes(1500)       // "1.5 KB"; the one built-in the UI can call
pick_file($path, title: "Open", types: ["png", "jpg"], from: "~/Pictures")
pick_files($paths)
pick_folder($folder)
pick_save_path($path, name: "export.csv", types: ["csv"])
open_window(settings)    close_window(settings)
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
- Only button actions, bindings, `download`, `cancel_download` and the `pick_…` dialogs change state.
