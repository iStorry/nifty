# UI elements

Every built-in element, with all its settings. Settings marked *required* must be
given; the rest are optional.

**Kinds of value:**

- **text**: any expression that gives a `string`; stays up to date, e.g. `"{count} left"`.
- **fixed text**: a string written directly, without `{…}`, e.g. `"Todos"`.
- **number**: a number written directly, between `0` and `10000`, e.g. `12`.
- **fixed int**: a whole number written directly, between `-1000000000` and
  `1000000000`, e.g. `-10`.
- **int**: any expression that gives an `int`; stays up to date, e.g. `item.percent`.
- **list of text**: any expression that gives a `[string]`; stays up to date.
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
| `backdrop` | option | `.standard` *(default)*, `.translucent`, `.solid` |

`width` and `height` are the size of the content area. The window can't be made
narrower than its content needs: it opens wider if the content doesn't fit, and the user
can't drag it narrower. It can be made as short as the user likes, because a long list
would otherwise make the window taller than the screen.

On macOS the title bar is transparent and the content area runs underneath it, the way
Finder and Mail look. Content still starts below the title bar; only a
[surface](#column) that starts at the top of the window reaches up under it. On Windows
the title bar sits above the content area, as usual.

**Backdrops** are drawn by the system, so they cost the app nothing:

| `backdrop` | macOS | Windows 11 |
|---|---|---|
| `.standard` | the usual window | Mica in the title bar and frame |
| `.translucent` | the whole window shows a blurred view of what's behind it, where no [surface](#column) covers it | Acrylic in the title bar and frame |
| `.solid` | the usual window | no material: a solid title bar and frame |

On Windows the material stays in the title bar and frame, and the content area keeps its
solid Fluent colors. The system's own controls can't yet draw on a see-through content
area. Windows 10, Windows 11 before version 22H2, and high contrast themes draw every
backdrop as `.solid`.

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

## `scroll`

Stacks its children top to bottom, like [`column`](#column), and scrolls them up and down
when they are taller than the space it has.

```nf
scroll(spacing: 8, padding: 12) {
  for item in downloads {
    DownloadRow(item: item)
  }
}
```

Same settings as [`column`](#column), but `grow`: a scroll area always takes the space
its parent has left along its direction, like [`spacer`](#spacer), and stretches across
it. It adds nothing to its parent's size, so put it straight in a window, or in a stack
that has space to give, such as one with `grow: true`; in a stack that fits its content
it gets no space. A `surface` stays put behind the content as it scrolls.

| | macOS | Windows |
|---|---|---|
| Scrolling | the trackpad, mouse wheel or scroll bar, with the system's elastic bounce | the mouse wheel, by as many lines a notch as the user set, or the scroll bar |
| Scroll bar | floats over the content, and shows while scrolling; with **Show scroll bars: Always**, it takes a strip beside the content and always shows | takes a strip beside the content, and always shows, greyed out when there's nothing to scroll |

On Windows, moving the keyboard focus to a control that's scrolled out of view doesn't
yet scroll it into view.

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
| `selected` | bool | shows the button as selected while it holds; stays up to date |

| Style | Looks like |
|---|---|
| `.normal` | a standard push button |
| `.primary` | the window's main action, in the accent color: pressing **Return** clicks it, unless the focus is in a text field with its own action |
| `.plain` | borderless until the pointer is over it, in the accent color, for quieter actions such as those in a row |
| `.icon` | borderless until the pointer is over it, showing only its icon; the title becomes its tooltip and what screen readers say, so always give one |

With several primary buttons on Windows, **Return** clicks the newest one still showing.

**Picking one of a few choices.** Give each choice a button whose `selected` says whether
it is the current one, and whose action picks it:

```nf
state filter = "all"

row(spacing: 4) {
  button("All", selected: filter == "all") { filter = "all" }
  button("Active", selected: filter == "active") { filter = "active" }
  button("Done", selected: filter == "done") { filter = "done" }
}
```

Clicking a button never selects it by itself: it shows whatever `selected` says once its
action has run. A `.primary` button can't take `selected`, since it is the window's
default action.

| | macOS | Windows |
|---|---|---|
| Selected `.normal` | the system's on-off push button, shown on | Fluent's checked toggle button, in the accent color |
| Selected `.plain`, `.icon` | an on-off accessory bar button, shown on | a subtle fill, with the title or icon in the accent color |
| Screen readers | an on-off button | a push-like checkbox, announced as pressed while selected |

**Block:** the action (*required*). It may change state.

---

## `text_field`

A text box, bound to a `string`: one line, or several that wrap and scroll.

```nf
text_field($draft, placeholder: "What needs doing?")
text_field($notes, lines: 4)
```

| Setting | Kind | |
|---|---|---|
| *(first)* value | binding to a `string` | *required* |
| `placeholder` | fixed text | grey hint shown while empty; one-line fields only |
| `lines` | fixed int | how many lines tall, from `1` *(default)* to `100` |

A one-line field can take a block, which runs when the user presses **Return** in it,
in place of pressing the window's `.primary` button:

```nf
text_field($query, placeholder: "Search") { search() }
```

A field with more than one line wraps its text, scrolls when the text is longer than it
is, and takes **Return** as a new line. Lines in the bound text end in `\n` on both
platforms. Neither platform's multi-line text box shows a placeholder, so `placeholder`
goes only with one-line fields.

| | macOS | Windows |
|---|---|---|
| One line | the system's text field | the system's edit control |
| Several lines | a text view in a bordered scroll view, as in Mail's message body | a multi-line edit control with a scroll bar |
| **Tab** in several lines | types a tab; **Ctrl+Tab** moves to the next control | the same |

**Block:** the action **Return** runs, for a one-line field; optional. It may change
state.

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

## `switch`

Turns a `bool` on and off, like a [`checkbox`](#checkbox), with the switch at the end of
its title, as in a settings window.

```nf
switch(on: $notify, title: "Notify me when downloads finish")
```

| Setting | Kind | |
|---|---|---|
| `on` | binding to a `bool` | *required* |
| `title` | text | shown at the start of the row |

The title sits at the start of the row and the switch at its end, so in a `column` with
the default `align: .stretch` the switches of several rows line up.

| | macOS | Windows |
|---|---|---|
| Switch | the system's `NSSwitch` | Fluent's toggle switch, drawn by Nifty on a real checkbox, so screen readers announce it as a checkbox |

**Block:** none.

---

## `slider`

Picks a whole number by dragging a knob along a track.

```nf
slider($volume, min: 0, max: 10)
```

| Setting | Kind | |
|---|---|---|
| *(first)* value | binding to an `int` | *required* |
| `min` | fixed int | the left end; default `0` |
| `max` | fixed int | the right end; default `100`, and no lower than `min` |

The knob slides smoothly and writes the whole number nearest it each time that number
changes. A slider has no natural width: it stretches across a `column` with the default
`align: .stretch`, and is 100 points wide where it can't stretch.

| | macOS | Windows |
|---|---|---|
| Slider | the system's `NSSlider` | the system's trackbar, drawn by Nifty in Fluent's style; the arrow keys and Page Up and Page Down move it too |

**Block:** none.

---

## `picker`

Picks one string from a list, in a pop-up menu.

```nf
state speeds = ["Unlimited", "1 MB/s", "500 KB/s"]
state speed = "Unlimited"

picker($speed, options: speeds)
```

| Setting | Kind | |
|---|---|---|
| *(first)* selection | binding to a `string` | *required*; the option shown, and where a pick goes |
| `options` | list of text | *required*; stays up to date |

The picker shows the option equal to `selection`, or nothing when no option is. It is as
wide as its widest option, so it doesn't change size as the pick changes.

| | macOS | Windows |
|---|---|---|
| Picker | the system's pop-up button | a button drawn by Nifty as Fluent's combo box, opening Win32's own pop-up menu; screen readers announce it as a button named by the option it shows |

Win32's menus follow the system's light look, also in dark mode, as other classic Windows
menus do.

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

## `image`

Shows the image in a file, as large as fits its frame, keeping its shape.

```nf
image("~/Pictures/cover.png", width: 120)
image(item.thumbnail, width: 48, height: 48)
```

| Setting | Kind | |
|---|---|---|
| *(first)* path | text | *required*; where the file is; stays up to date |
| `width` | number | a fixed width; the height follows the image's shape |
| `height` | number | a fixed height; the width follows the image's shape |

With both `width` and `height`, the image fits inside that box, centred, keeping its
shape. With neither, it shows at its own size, a pixel to a point.

The path must be a full one, such as `/Users/ada/Pictures/a.png` or
`C:\Users\ada\Pictures\a.png`, or start with `~` for the home folder. Images that come
with an app, found next to its code, aren't supported yet, so a relative path written
directly is an error. A path that names no file, or a file that isn't an image, shows
nothing, at the size given; so does an empty path. The image is read again whenever the
path changes.

| | macOS | Windows |
|---|---|---|
| Formats | whatever the system reads: PNG, JPEG, GIF, HEIC, TIFF, PDF and more | what the Windows Imaging Component reads: PNG, JPEG, GIF, BMP, TIFF and ICO, and more with codecs installed |
| Drawing | the system's image view, scaling smoothly | scaled smoothly by the Windows Imaging Component and blended with its transparency |

**Block:** none.

---

## `progress`

A bar that shows how far along some work is. With a `value` it fills to that percentage;
without one it animates to show that work is under way but can't say how far.

```nf
progress(value: item.percent)
progress()
```

| Setting | Kind | |
|---|---|---|
| `value` | int | how far along, from `0` (empty) to `100` (full); stays up to date |

A `value` below `0` shows an empty bar, and one above `100` a full one. Whether a bar has
a `value` is fixed where it's written; to switch between the two, show one or the other
with `if`.

A progress bar has no natural width: it stretches across a `column` with the default
`align: .stretch`, and is 100 points wide where it can't stretch.

| | macOS | Windows |
|---|---|---|
| With `value` | the system's bar, in the accent color | the system's progress bar |
| Without | the bar's animated stripe | the bar's marquee, sweeping across it |

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

## `context_menu`

A menu shown when the user right-clicks the stack it is in, written among the stack's
children.

```nf
for item in downloads {
  row() {
    context_menu() {
      menu_item("Download Again") { start(item.url) }
      divider()
      menu("Copy") {
        menu_item("Address") { copied = item.url }
      }
    }
    label(item.url)
  }
}
```

No settings. It takes no room and draws nothing: it covers its whole stack, padding
included, wherever it is written among the children. Where stacks with context menus
sit inside each other, a right-click shows the menu of the innermost one under the
pointer. A stack has one `context_menu`, and one can't be written straight inside a `for`,
where it would cover the stack around the loop once per item; put it in the `row` or
`column` the loop shows for each item instead.

Its block holds the same entries as a [`menu`](#menu): `menu_item`s, `divider()`s and
submenus. The items take no `shortcut`, since a shortcut can't say which stack it means,
and their actions see the names around the menu, such as a `for` loop's item.

| | macOS | Windows |
|---|---|---|
| Opens on | a right-click or a Control-click | a right-click, or Shift+F10 or the Menu key while a control in the stack has the keyboard, which shows it at the control |
| Text fields and selectable labels | keep their own menu, with Copy and the like | keep their own menu, with Copy and the like |

**Block:** the entries (*required*).

---

## `menu`

A menu of the app's menu bar, written at the top level beside its windows, or a submenu,
written inside another menu.

```nf
menu("Downloads") {
  menu_item("Pause All", shortcut: "shift+p") { pause_all() }
  divider()
  menu("Sort By") {
    menu_item("Name") { sort = "name" }
  }
}
```

| Setting | Kind | |
|---|---|---|
| *(first)* title | fixed text | *required* |

The block holds the menu's entries, in order: `menu_item`s, `divider()`s, which draw a
line between groups of items, and submenus. Menus can't hold `if` or `for`; an item's
action decides what to do when it runs.

| | macOS | Windows |
|---|---|---|
| Where | the menu bar at the top of the screen: the app menu with **Quit**, your first menu, the **Edit** menu with **Undo**, **Cut**, **Copy**, **Paste** and **Select All**, then the rest of your menus | the menu bar of every window, holding your menus in order; text fields take their editing shortcuts themselves |
| Look | the system's own | Win32's own, which stays light in dark mode, as classic Windows menu bars do |

**Block:** the entries (*required*).

---

## `menu_item`

An item of a `menu`, which runs its block when the user chooses it or presses its
shortcut.

```nf
menu_item("New Download", shortcut: "n") { new_download() }
```

| Setting | Kind | |
|---|---|---|
| *(first)* title | fixed text | *required* |
| `shortcut` | fixed text | a key, after `shift+` or `alt+` if any, such as `"n"`, `"shift+n"` or `"alt+return"` |

A shortcut always uses the platform's command key: Cmd on macOS, Ctrl on Windows. Keys
are the letters, the digits, `, . / ; ' [ ] - =`, and `return`, `delete`, `up`, `down`,
`left` and `right`; `alt` is Option on macOS. The shortcuts the system's own commands
use, `q` (Quit), `z` and `shift+z` (Undo and Redo), `x`, `c`, `v` and `a` (Cut, Copy,
Paste and Select All), are kept for them, and two items can't share a shortcut.

The shortcut shows beside the item's title, as each platform writes it: ⇧⌘N on macOS,
Ctrl+Shift+N on Windows. It works whenever one of the app's windows has the keyboard. On
Windows, a punctuation key that the user's keyboard layout doesn't have gives no
shortcut, though the item still shows it.

**Block:** the action (*required*). It may change state.

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
