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

## Spacing and alignment

Both `column` and `row` take the same optional settings. These four place their
children:

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

## Sizes

A stack fits its content unless you say otherwise:

| Setting | What it does | Example |
|---|---|---|
| `width` | a fixed width, in points | `width: 200` |
| `max_width` | fill the width on offer, up to this many points | `max_width: 600` |
| `grow` | take the space the parent has left along its direction | `grow: true` |

A window can't be made narrower than its content, so a fixed-width sidebar and a
text field never get squashed.

## Sidebars, panes and cards

`surface:` draws a stack on one of the system's own backgrounds. Here is a whole app
with a sidebar, a main pane and a card:

```nf
{{#include ../../../fixtures/sidebar.nf}}
```

- `.sidebar` is the side panel. On macOS it's translucent and runs up under the title
  bar, like Finder's.
- `.content` is the main pane, in the color content is read on.
- `.card` is a rounded, outlined panel that groups related content.

`grow: true` lets the main pane take the rest of the window. `max_width: 480` inside a
pane with `align: .center` keeps the text a comfortable width, centred, when the window
is maximized. See [`column`](../reference/elements.md#column) for how each surface
looks on each platform.

## Text sizes

`label` has four text styles. They use your system's own fonts, so they follow
accessibility text sizes and dark mode automatically.

| Style | Use it for |
|---|---|
| `.title` | the main heading |
| `.headline` | a bold section heading |
| `.body` *(default)* | normal text |
| `.caption` | small, quieter text |

To change a label's color, use `color:` with one of the system's colors, such as
`.secondary`, `.accent` or `.danger`. To place its text, use `align:` with `.start`,
`.center` or `.end`. See [`label`](../reference/elements.md#label) for the full list.
