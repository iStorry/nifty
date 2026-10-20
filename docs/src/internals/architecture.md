# How Nifty works

This page follows a `.nf` file from text on disk to a window on screen. You don't need
any of this to *use* Nifty, but it's the map to read before changing it.

## The big picture

```text
        main.nf and the project's other .nf files
                    │
                    ▼
   ┌────────────────────────────────┐
   │ nifty-syntax                   │  text → tokens → syntax tree
   │   lexer, parser                │  (+ syntax errors)
   └────────────────────────────────┘
                    │  ast::File
                    ▼
   ┌────────────────────────────────┐
   │ nifty-check                    │  names, types, element arguments,
   │   checker                      │  and which state each UI part reads
   └────────────────────────────────┘
                    │  hir::Program
          ┌─────────┴──────────┐
          ▼                    ▼
   ┌──────────────┐     ┌──────────────┐
   │ nifty-interp │     │  nifty-lsp   │  editor features
   │   runs it    │     │ (nf lsp)     │  (diagnostics, completion, …)
   └──────────────┘     └──────────────┘
          │  creates and updates widgets
          ▼
   ┌────────────────────────────────┐
   │ nifty-runtime                  │  widget tree + flexbox layout,
   │   Ui, Backend trait, layout    │  same on every OS
   └────────────────────────────────┘
          │  Backend trait
          ▼
   ┌────────────────────────────────┐
   │ nifty-platform                 │  picks the backend for this OS
   └────────────────────────────────┘
          │
     ┌────┴───────────────┐
     ▼                    ▼
   ┌──────────────┐     ┌───────────────┐
   │ nifty-macos  │     │ nifty-windows │
   │   AppKit     │     │   Win32       │
   └──────────────┘     └───────────────┘
```

`nifty-cli` (the `nf` program) ties these together: it reads the project, calls the
syntax and check stages, prints errors, and starts the interpreter or the language
server. It also packages apps (`nf build`) and watches files for live reload
(`nf dev`); both are described below.

## Stage 1: Syntax (`nifty-syntax`)

**Input:** source text. **Output:** a syntax tree (`ast::File`) plus a list of
diagnostics.

1. The **lexer** cuts the text into tokens: names, numbers, strings, operators,
   keywords and line breaks. Line breaks are tokens because they end statements; the
   lexer drops the ones inside `( )` and `[ ]` or after a binary operator.
2. The **parser** builds the tree from the tokens. It never gives up: when something
   doesn't parse, it records an error, skips ahead, and keeps going, so you see every
   mistake at once, and the editor still understands the rest of the file.

## Stage 2: Checking (`nifty-check`)

**Input:** the syntax tree. **Output:** a checked program (`hir::Program`) plus
diagnostics.

The checker:

- **resolves names**, so every use of `count` points at the one `state count`;
- **works out types** and makes sure they fit, such as not adding `int` to `float`;
- **checks elements**: every `label`, `button` and the rest gets the arguments it
  accepts, using the single table in `elements.rs`;
- **enforces the rules** that keep apps predictable, such as "showing the UI can't
  change state";
- **records dependencies**: for each dynamic part of the UI (a label's text, an `if`,
  a `for`, a view), it records which state that part reads, including state read inside
  functions it calls.

That last step is what makes Nifty's updates precise. It's all worked out before the
app runs.

## Stage 3: Running (`nifty-interp`)

**Input:** the checked program and a platform backend.

The interpreter:

1. sets every `state` to its starting value;
2. **mounts** each window: walks the UI description and asks the runtime to create a
   native widget for each element;
3. waits for events. When you click a button, it runs the action, and notes which state
   changed;
4. **updates** only the UI parts whose recorded dependencies changed: new text for a
   label, or rebuilding an `if`, `for` or view region in place.

### Live reload (`nf dev`)

`nf dev` runs the app as a `LiveApp`, whose program can be swapped while it runs. A file
watcher wakes the UI thread when a `.nf` file in the project folder is saved; `nf dev`
then re-reads and re-checks the source. If it has errors, they're printed and the old
version keeps running. If not, the new program replaces the old one: each state the new
program declares with the same name and an equivalent type keeps its value (see
`machine/carry.rs`), the rest start from their starting values, and the windows are
rebuilt from those values.

### Built apps (`nf build`)

`nf build` checks the project, then compiles the checked program to machine code with
`nifty-codegen`, which uses [Cranelift](https://cranelift.dev). It compiles every
function, every state's starting value, and every text, binding, action and region of
the UI, and emits a descriptor of the windows and views as data. It links that object
file with `nifty-rt`, a static library installed next to `nf`, using the system's linker
(`cc` on macOS, the MSVC linker on Windows), and writes the executable into a macOS
`.app` bundle or a Windows folder in `build/`.

When the app starts, `nifty-rt` reads the descriptor and shows the windows through the
same engine the interpreter uses, calling the compiled functions where the interpreter
would evaluate checked code. Compiled code calls back into `nifty-rt` for anything
beyond arithmetic: strings, lists and structs, which are reference-counted and copied
when changed, state, downloads and errors. The tests in `nifty-codegen` run programs
compiled and interpreted side by side and check they behave the same.

`nf` has only its own system's runtime library and linker, so it builds apps for the
system it runs on.

## Stage 4: Widgets and layout (`nifty-runtime`, `nifty-macos`, `nifty-windows`)

`nifty-runtime` is the part that's the same on every operating system. It keeps a tree
of widgets, lays them out with flexbox (using the [Taffy](https://github.com/DioxusLabs/taffy)
library), and talks to the OS through one trait, `Backend`.

Each OS gets its own crate that implements `Backend` with that system's real widgets:
`nifty-macos` (AppKit) and `nifty-windows` (Win32 common controls). `nifty-platform`
picks the right one at compile time. Because the trait lists every widget operation, a
new element can't compile until both backends implement it. New backends, such as Linux
(GTK), slot in next to them without touching anything above.

The `Backend` trait also has a **waker**: a way for another thread, such as `nf dev`'s
file watcher, to ask the UI thread to run something. Native toolkits only allow UI work
on their main thread, so this is how outside events get in.

File dialogs go through the waker too. Code that calls `pick_file` only asks for one;
once the event has finished, the waker's handler shows it with the backend's
`show_file_dialog`, outside any event, because Windows runs a modal loop there that
handles other events. The answer is queued and the waker runs again to write it into
state, the same way on both systems.

## Editor support (`nifty-lsp`)

The language server finds the project each open file belongs to, the nearest folder
above it with a `main.nf`, and analyses all of the project's files together, using the
text of open files and the saved text of the rest. It reuses stages 1 and 2 exactly as
`nf check` does, then answers
editor questions from the result: errors, completions, signature help, hover, semantic
colors, the outline, go to definition and references. Because it uses the same checker, the editor and
`nf check` can never disagree.

## Where to read next

- [Browse the source code](source-code.md): every crate and file, with links.
- [Language specification](../reference/language.md): the exact rules the checker
  enforces.
