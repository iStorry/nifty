# Reading error messages

Nifty tries hard to explain problems in plain words and to suggest a fix. This page
shows how to read an error, then lists the ones you're most likely to meet.

## Anatomy of an error

```text
Error: `label` shows text; turn this `int` into text with "{…}"
   ╭─[ hello/main.nf:5:11 ]
   │
 5 │     label(count)
───╯
1 error
```

- **The first line** says what's wrong, and usually how to fix it.
- **`hello/main.nf:5:11`** is the file, the line (5) and the column (11).
- **The code below** is the line itself.

Nifty reports every problem it finds in one go, not just the first, and the count is
at the bottom. In an editor with [Nifty support](../getting-started/editors.md), the
same errors appear as squiggles while you type.

## When errors appear

| When | What finds it | Example |
|---|---|---|
| **Before running** | `nf check`, `nf run`, `nf dev` and `nf build` | a typo in a name, the wrong type |
| **While running** | `nf run` and `nf dev` | dividing by zero, a list item that isn't there |

Most mistakes are caught before running. When an error happens *while* running, `nf run`
stops the app and points at the line, just like a check error. `nf dev` points at the
line too, but keeps the app open so you can save a fix.

## Common errors

### `` `label` shows text; turn this `int` into text with "{…}" ``

You gave `label` a number. Put it in a string with interpolation:

```nf
label(count)          // ✗
label("{count}")      // ✓
```

### `` can't use `+` with `string` and `int` ``

You joined text and a number with `+`. Use interpolation instead:

```nf
label("Count: " + count)    // ✗
label("Count: {count}")     // ✓
```

### `` can't use `+` with `int` and `float` ``

`int` and `float` don't mix. Write both the same way: `2.0 * 1.5`, or `2 * 3`.

### ``expected end of line, found `{`; put each statement on its own line``

Usually a missing `()` on an element:

```nf
column { … }      // ✗
column() { … }    // ✓
```

### `` `button` needs a `{ … }` block with what it does ``

Every button needs an action, even an empty one:

```nf
button("Later")        // ✗
button("Later") { }    // ✓
```

### `` `text_field` needs a two-way binding to a `string`, such as `$name` ``

Controls that edit a value need a `$`. See [Two-way bindings](bindings.md).

```nf
text_field(draft)      // ✗
text_field($draft)     // ✓
```

### `` unknown name `count` ``

There's no state, variable or parameter with that name. Check the spelling, and check
that you declared it with `state` or `let`.

### `` can't tell what this empty list holds; add a type such as `: [int]` ``

An empty list needs its type written out:

```nf
state todos = []              // ✗
state todos: [Todo] = []      // ✓
```

### `` only declarations and `window(…)` can appear at the top level ``

Outside any window or function, you can only declare things: `state`, `fn`, `view`,
`struct` and `window`. Use `state` (not `let`) for top-level values.

### `` only elements, `if` and `for` can appear here; put logic in a function or a button's action ``

You wrote a statement like `count = 0` directly in a window or view. Move it into a
button's action or a function.

### `showing the UI can't change state; do that in a button's action`

Something used to *show* the UI tried to change state, often a function call inside a
label. See [State](state.md#showing-state-never-changes-it).

### `a state's starting value can't use other state`

`state b = a` isn't allowed. Use a function that computes `b` from `a` instead.

### `` `remaining` can reach its end without returning a `int` ``

Some path through the function finishes without a `return`. Often an `if` without an
`else`; add a `return` at the end.

### `there is no item 3; the list has 2` *(while running)*

You read a list position that doesn't exist. Positions start at `0`, so a list of two
has items `0` and `1`. Check `items.len()` first.

### `can't divide by zero` *(while running)*

Check the number you divide by isn't `0` before dividing.

## Still stuck?

Open an [issue on GitHub](https://github.com/istorry/nifty/issues) with your code and
the error. Confusing error messages count as bugs.
