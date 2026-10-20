# Your first app

Let's create an app, run it, and change it. You'll need `nf` installed
([Install Nifty](install.md)).

## Create it

In your terminal, go to a folder where you keep projects and run:

```bash
nf create hello
```

`nf` makes a new folder for you:

```text
Created hello/main.nf

Run it with:
  nf run hello
```

A Nifty **project** is just a folder with a file called `main.nf` in it. That file is
your whole app.

> App names must start with a letter and use only letters, digits, `-` and `_`.

## Run it

```bash
nf run hello
```

A window opens with a counter. Click the buttons and watch the number change. Close the
window (or press **⌘Q** on macOS, **Alt+F4** on Windows) to stop the app.

## Look inside

Open `hello/main.nf` in any text editor. You'll see:

```nf
{{#include ../../../crates/nifty-cli/templates/main.nf:1:2}}
window("hello", width: 320, height: 200) {
{{#include ../../../crates/nifty-cli/templates/main.nf:4:}}
```

Read it top to bottom:

1. **`state count = 0`** creates a value called `count` that starts at `0`. *State* is
   the data your app remembers. When it changes, the screen updates.
2. **`window("hello", …)`** makes a window with the title "hello", 320 points wide and
   200 tall.
3. **`column(…)`** stacks things top to bottom. **`row(…)`** puts them side by side.
4. **`label("Clicked {count} times", …)`** shows text. The `{count}` part is replaced by
   the current value of `count`.
5. **`button("+") { count += 1 }`** is a button. The code in `{ … }` runs when you click
   it.

## Change it

Let's add a **Reset** button. Inside the `row`, after the `+` button, add one line:

```nf
    row(spacing: 8) {
      button("−") { count -= 1 }
      button("+", style: .primary) { count += 1 }
      button("Reset") { count = 0 }
    }
```

Save the file and run it again:

```bash
nf run hello
```

You now have three buttons.

## Change it while it runs

Quitting and running again gets old fast. Use `nf dev` instead:

```bash
nf dev hello
```

The app opens as before, but now `nf dev` watches your file. Click **+** a few times,
then change `"Clicked {count} times"` to `"Count: {count}"` and save. The window
updates straight away, **and the count stays where it was**.

If you save a mistake, `nf dev` prints the error and keeps showing the last version that
worked, so you can fix it and save again. Press **Ctrl+C** in the terminal (or quit the
app) when you're done.

> **Tip:** Keep `nf dev` running in one window and your editor in another. It's the
> fastest way to build a Nifty app.

## Check for mistakes

Try breaking something on purpose. Change the label to `label(count)` (without the
quotes) and run:

```bash
nf check hello
```

Nifty tells you what's wrong and where:

```text
Error: `label` shows text; turn this `int` into text with "{…}"
   ╭─[ hello/main.nf:5:11 ]
   │
 5 │     label(count)
───╯
1 error
```

`nf check` looks for problems without opening any windows. `nf run` does the same check
first, so a broken app never starts half-working. Put the quotes back and you're good.

> **Tip:** Inside the project folder you can leave the name off: `cd hello` then just
> `nf run` or `nf check`.

## What's next

- Set up [syntax highlighting and live errors in your editor](editors.md).
- Learn properly with the [counter tutorial](../tutorials/counter.md).
- Turn your app into a real `.app` to share: [Share your app](share.md).
