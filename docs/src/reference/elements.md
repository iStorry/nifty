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
| `style` | option | `.body` *(default)*, `.title`, `.headline`, `.caption` |
| `color` | option | `.primary`, `.secondary`, `.tertiary`, `.accent`, `.success`, `.warning`, `.danger` |
| `align` | option | `.start` *(default)*, `.center`, `.end` |
| `selectable` | flag | `false` *(default)*; `true` lets the user select and copy the text |

`color` defaults to `.secondary` for `.caption` and `.primary` for every other style.
The colors are the system's own, so they follow dark mode and the user's accent color.

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
```

| Setting | Kind | |
|---|---|---|
| *(first)* title | text | *required* |
| `style` | option | `.normal` *(default)*, `.primary` |

A `.primary` button is the window's main action: it's drawn in the accent color, and
pressing **Return** clicks it.

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

> **Where this comes from:** the list of elements is defined once in
> [`crates/nifty-check/src/elements.rs`](https://github.com/istorry/nifty/blob/main/crates/nifty-check/src/elements.rs).
> The checker and the editor's autocomplete both read it, so they always agree.
