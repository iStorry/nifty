# Layout

Nifty arranges your UI with two containers:

- **`column`** stacks children **top to bottom**.
- **`row`** places children **left to right**.

Put rows inside columns (and columns inside rows) to build any layout.

```nf
column(spacing: 12, padding: 20) {
  label("Title", style: .title)
  row(spacing: 8) {
    button("Cancel") { }
    button("OK", style: .primary) { }
  }
}
```

## The four settings

Both `column` and `row` take the same four optional settings:

| Setting | What it does | Example |
|---|---|---|
| `spacing` | space between children, in points | `spacing: 8` |
| `padding` | space around the edge, in points | `padding: 20` |
| `align` | where children sit **across** the stack | `align: .center` |
| `justify` | where children sit **along** the stack | `justify: .space_between` |

Numbers must be between `0` and `10000`.

## `align` and `justify`, explained

The easy way to remember: **justify** works in the direction the stack flows;
**align** works the other way.

| | `justify` moves things… | `align` moves things… |
|---|---|---|
| `column` | up and down | left and right |
| `row` | left and right | up and down |

**`align`** options:

| Option | Children… |
|---|---|
| `.stretch` *(default)* | stretch to fill the width of a column (or height of a row) |
| `.start` | sit at the left of a column (or top of a row) |
| `.center` | sit in the middle |
| `.end` | sit at the right of a column (or bottom of a row) |

**`justify`** options:

| Option | Children… |
|---|---|
| `.start` *(default)* | bunch up at the start |
| `.center` | bunch up in the middle |
| `.end` | bunch up at the end |
| `.space_between` | spread out, with the first and last at the edges |

## Pushing things apart with `spacer`

`spacer()` takes up all the leftover space. Use it to push things to opposite ends:

```nf
row() {
  label("Todos", style: .title)
  spacer()
  label("3 left", style: .caption)
}
```

The title sits on the left and the count on the right.

## Separating sections with `divider`

`divider()` draws a thin line across the stack, to separate groups of elements.

## Centering everything

To put content in the middle of a window, center on both axes:

```nf
column(align: .center, justify: .center) {
  label("Right in the middle")
}
```

## Text sizes

`label` has four text styles. They use your system's own fonts, so they follow
accessibility text sizes automatically, and dark mode on macOS.

| Style | Use it for |
|---|---|
| `.title` | the main heading |
| `.headline` | a bold section heading |
| `.body` *(default)* | normal text |
| `.caption` | small, quieter text |

To change a label's color, use `color:` with one of the system's colors, such as
`.secondary`, `.accent` or `.danger`. To place its text, use `align:` with `.start`,
`.center` or `.end`. See [`label`](../reference/elements.md#label) for the full list.
