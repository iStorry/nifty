# Gallery

One window with every built-in element and every style, so you can see them side by
side and copy the one you need.

```nf
{{#include ../../../examples/gallery/main.nf}}
```

**Run it:**

```bash
nf run examples/gallery
```

**Source:** [`examples/gallery/main.nf`](https://github.com/istorry/nifty/blob/main/examples/gallery/main.nf)

## What's in it

| Section | Shows |
|---|---|
| Text styles | `label` with each `style`: `.title`, `.headline`, `.body`, `.caption` |
| Text colors | each `color`: `.primary` through `.danger` |
| Text alignment | `align: .start`, `.center` and `.end` on a full-width label |
| Selectable text | `selectable: true` |
| Buttons | `.normal` and `.primary` buttons changing a counter |
| Text field | `text_field` with a `placeholder`, and an `if` / `else` on what's typed |
| Checkboxes | `checkbox` with and without a `title` |
| Layout | every `align` on a full-width `column`, so the button moves across it, and every `justify` on a `row` |

The header uses `spacer()` to push the caption to the right edge, and `divider()`
separates the sections. Each section heading is a small `view`.

A test checks that this example uses every element and every option, so when Nifty
gains a new one, the gallery gains it too. Every setting is listed in
[UI elements](../reference/elements.md).
