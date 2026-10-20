# UI elements

Every built-in element, with all its settings. Settings marked *required* must be
given; the rest are optional.

**Kinds of value:**

- **text**: any expression that gives a `string`; stays up to date, e.g. `"{count} left"`.
- **fixed text**: a string written directly, without `{…}`, e.g. `"Todos"`.
- **number**: a number written directly, between `0` and `10000`, e.g. `12`.
- **option**: one of a fixed set of `.names`, e.g. `.center`.
- **flag**: `true` or `false`, written directly.
- **binding**: a `$` place, e.g. `$draft`. See [Two-way bindings](../guide/bindings.md).

Unnamed settings come first, in order; named ones follow in any order.

---

## `window`

A window. Only at the top level of a file. An app needs at least one.

```nf
window("Todos", width: 360, height: 480) { … }
```

| Setting | Kind | |
|---|---|---|
| *(first)* title | fixed text | *required* |
| `width` | number | default `480` |
| `height` | number | default `320` |

`width` and `height` are the size of the content area. The window can't be made
narrower than its content needs: it opens wider if the content doesn't fit, and the user
can't drag it narrower. It can be made as short as the user likes, because a long list
would otherwise make the window taller than the screen.

On macOS the title bar is transparent and the content area runs underneath it, the way
Finder and Mail look. Content still starts below the title bar; only a
[surface](#column) that starts at the top of the window reaches up under it. On Windows
the title bar sits above the content area, as usual.

**Block:** the window's content (*required*).

---

## `column`

Stacks its children top to bottom. See [Layout](../guide/layout.md).

```nf
column(spacing: 12, padding: 20, align: .center) { … }
```

| Setting | Kind | |
|---|---|---|
| `spacing` | number | space between children |
| `padding` | number | space around the edge |
| `align` | option | `.stretch` *(default)*, `.start`, `.center`, `.end` |
| `justify` | option | `.start` *(default)*, `.center`, `.end`, `.space_between` |
| `surface` | option | `.none` *(default)*, `.sidebar`, `.content`, `.card`; what the stack is drawn on |
| `width` | number | a fixed width; without it the stack fits its content |
| `max_width` | number | the stack fills the width it's given, up to this |
| `grow` | flag | `false` *(default)*; `true` takes the space the parent has left along its direction |

**Surfaces** are drawn behind the stack's children with each platform's own materials, so
they follow dark mode and high contrast:

| Surface | Use it for | macOS | Windows |
|---|---|---|---|
| `.sidebar` | a window's side panel | translucent sidebar material that shows the desktop through it | a solid panel a shade apart from the window |
| `.content` | the window's main pane | the color content is read on: white, or dark in dark mode | Fluent's content layer color |
| `.card` | grouping related content | a rounded panel with a thin outline | a rounded panel with a thin outline |

A surface that starts at the top of the window reaches up under the macOS title bar, so a
sidebar runs the full height of the window. Its children still start below the title
bar.

`max_width` keeps text readable when a window is maximized: put the stack in a parent with
`align: .center` and it stays centred at that width, or narrower when the window is.

```nf
row() {
  column(surface: .sidebar, width: 200, padding: 12) { … }
  column(surface: .content, grow: true, align: .center) {
    column(max_width: 600, padding: 24) { … }
  }
}
```

**Block:** the children.

---

## `row`

Places its children left to right. Same settings as [`column`](#column).

```nf
row(spacing: 8, align: .center) { … }
```

**Block:** the children.

---

## `label`

Shows text.

```nf
label("{left} left", style: .caption)
label("Couldn't save", color: .danger, align: .center)
label(token, selectable: true)
```

| Setting | Kind | |
|---|---|---|
| *(first)* text | text | *required* |
| `style` | option | `.body` *(default)*, `.large_title`, `.title`, `.headline`, `.caption` |
| `color` | option | `.primary`, `.secondary`, `.tertiary`, `.accent`, `.success`, `.warning`, `.danger` |
| `align` | option | `.start` *(default)*, `.center`, `.end` |
| `selectable` | flag | `false` *(default)*; `true` lets the user select and copy the text |

`color` defaults to `.secondary` for `.caption` and `.primary` for every other style.
The colors are the system's own, so they follow the user's accent color, dark mode and
high contrast on macOS and Windows, and change while the app runs when the user switches.

`align` places the text within the label's frame, so it only shows when the label is
wider than its text, as it is in a `column` with the default `align: .stretch`. `.start`
and `.end` follow the reading direction: in a right-to-left language, `.start` is the
right edge.

**Block:** none.

---

## `button`

Runs its block when clicked.

```nf
button("Add", style: .primary) { add_todo() }
button("Add", icon: .plus) { add_todo() }
button("Delete", style: .icon, icon: .trash) { delete(todo.id) }
```

| Setting | Kind | |
|---|---|---|
| *(first)* title | text | *required* |
| `style` | option | `.normal` *(default)*, `.primary`, `.plain`, `.icon` |
| `icon` | option | an [icon](#icons) shown before the title |

| Style | Looks like |
|---|---|
| `.normal` | a standard push button |
| `.primary` | the window's main action, in the accent color: pressing **Return** clicks it |
| `.plain` | borderless until the pointer is over it, in the accent color, for quieter actions such as those in a row |
| `.icon` | borderless until the pointer is over it, showing only its icon; the title becomes its tooltip and what screen readers say, so always give one |

With several primary buttons on Windows, **Return** clicks the newest one still showing.

**Block:** the action (*required*). It may change state.

---

## `text_field`

A one-line text box, bound to a `string`.

```nf
text_field($draft, placeholder: "What needs doing?")
```

| Setting | Kind | |
|---|---|---|
| *(first)* value | binding to a `string` | *required* |
| `placeholder` | fixed text | grey hint shown while empty |

**Block:** none.

---

## `checkbox`

A tick box, bound to a `bool`.

```nf
checkbox(checked: $todo.done, title: todo.title)
```

| Setting | Kind | |
|---|---|---|
| `checked` | binding to a `bool` | *required* |
| `title` | text | the label next to the box |

**Block:** none.

---

## `nav_item`

A row of a sidebar list, like those in Finder or File Explorer: an icon, a title, and a
badge such as a count at the end. It's highlighted while `selection` holds its `value`,
and picking it puts `value` there.

```nf
state section = "inbox"

nav_item("Inbox", value: "inbox", selection: $section, icon: .inbox, badge: "{unread}")
nav_item("Sent", value: "sent", selection: $section, icon: .send)
```

| Setting | Kind | |
|---|---|---|
| *(first)* title | text | *required* |
| `value` | text | *required*; what `selection` holds while this item is picked |
| `selection` | binding to a `string` | *required*; shared by the items of one list |
| `icon` | option | an [icon](#icons), drawn in the accent color |
| `badge` | text | shown at the end in a quieter color; empty shows nothing |

Put nav items one after another in a `column(surface: .sidebar)`; they need no
`spacing`, as each row is the platform's own height.

| | macOS | Windows |
|---|---|---|
| Row | 28 points, Finder's sidebar row | 32 pixels at 100%, Fluent's navigation item |
| Selected | a rounded highlight in the system's selection color | a subtle highlight with an accent mark at its start |
| Under the pointer, pressed | a fainter highlight, and a stronger one while pressed | the same, in Fluent's subtle fills |

**Block:** none.

---

## `spacer`

Takes up all the leftover space in a `row` or `column`, pushing its neighbours apart.

```nf
spacer()
```

No settings. **Block:** none.

---

## `divider`

A thin line, to separate groups of elements.

```nf
divider()
```

No settings. **Block:** none.

---

## Icons

`button` and `nav_item` take `icon:` with one of these:

`.plus`, `.trash`, `.star`, `.star_fill`, `.check`, `.check_circle`, `.circle`, `.home`, `.inbox`, `.archive`, `.list`, `.calendar`, `.sun`, `.flag`, `.close`, `.gear`, `.search`, `.mail`, `.send`, `.folder`, `.clock`, `.tag`, `.person`, `.document`, `.download`, `.heart`, `.bell`, `.pencil`, `.share`, `.info`

They come from the system's own icon set: SF Symbols on macOS, and Segoe Fluent Icons on
Windows 11 (Segoe MDL2 Assets on Windows 10). Nothing is added to your app, and icons
follow the system's look and the user's text size. On Windows, `.inbox` shows the same
envelope as `.mail`, as the Windows icon fonts have no inbox tray.

---

> **Where this comes from:** the list of elements is defined once in
> [`crates/nifty-check/src/elements.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-check/src/elements.rs).
> The checker and the editor's autocomplete both read it, so they always agree.
