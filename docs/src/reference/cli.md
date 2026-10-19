# The `nf` command

`nf` is the one tool you need for Nifty. Run `nf help` to see a summary.

A **project** is a folder with a `main.nf` file in it. Anywhere a command takes a
`[project]`, you can give a project folder, a single `.nf` file, or nothing at all to
use the current folder.

## `nf create <name>`

Makes a new folder called `<name>` with a starter app in `main.nf`.

```bash
nf create my-app
```

The name becomes the folder name and the window title. It must start with a letter and
use only letters, digits, `-` and `_`. `nf create` won't overwrite a folder that
already exists.

## `nf run [project]`

Checks the app, and if there are no errors, runs it with native windows.

```bash
nf run            # the project in this folder
nf run my-app     # a project folder
nf run demo.nf    # a single file
```

The app runs until you quit it (**⌘Q**). If it hits an error while running, it stops and
shows where.

## `nf dev [project]`

Like `nf run`, but it **reloads the app every time you save**. Keep it running while
you work, and each change shows up in the open window within a moment.

```bash
nf dev my-app
```

```text
my-app/main.nf: started
watching my-app for changes; quit the app or press Ctrl-C to stop
my-app/main.nf: reloaded
```

- **Your state is kept.** If you've typed three todos and then change a label's text,
  the todos are still there after the reload. A `state` keeps its value as long as the
  new version still declares it with the same name and the same type. Rename it or
  change its type, and it starts again from its starting value.
- **Mistakes don't close the app.** If you save code with errors, `nf dev` prints them
  and keeps showing the last version that worked. Fix the code, save, and it reloads.
  If the very first version has errors, it waits for you to save a fix.
- **Errors while running don't stop it either.** If a button's action hits an error,
  `nf dev` prints it and the app keeps running, so your next save can fix it. (`nf run`
  stops instead.)
- Saving without changing anything doesn't reload.

Stop it by quitting the app or pressing **Ctrl+C** in the terminal.

## `nf check [project]`

Looks for mistakes without running anything, and lists every one it finds.

```bash
nf check
```

```text
my-app/main.nf: no problems found
```

Handy in scripts and CI, since nothing opens on screen.

## `nf lsp`

Runs the language server, which gives editors errors-as-you-type, autocomplete, go to
definition and more. Your editor starts this for you; you don't run it by hand. See
[Set up your editor](../getting-started/editors.md).

## `nf build [project] [--macos | --windows | --linux]`

Checks the app, then packages it as a native app you can give to other people.

```bash
nf build todos
```

```text
built todos/build/todos.app
```

- The app goes in a `build` folder next to `main.nf`, named after the project folder
  (or after the file, for a single `.nf` file). Building again replaces it.
- It builds for the system you're on, which is also the default with no flag: on macOS,
  a `.app` bundle; on Windows, a `todos` folder holding `todos.exe` and the project.
  There's no Linux backend yet, so `--linux` doesn't work.
- If the app has errors, they're shown and nothing is built.

See [Share your app](../getting-started/share.md) for a walkthrough.

## `nf version`

Prints the version of `nf`, such as `nf 0.1.0-alpha.1`. `nf --version` works too.

## Exit codes

For scripts:

| Code | Meaning |
|---|---|
| `0` | everything worked |
| `1` | your program has errors (found by checking, or while running) |
| `2` | the command itself was wrong, e.g. a typo in the command or a missing folder |
