# Welcome to Nifty

<div class="hero">
<p class="tagline">Nifty is a small, friendly programming language for building <strong>real desktop apps</strong>, with real buttons, text fields and windows from your operating system.</p>
</div>

Here is a complete Nifty app. It's a window with a number and two buttons:

```nf
state count = 0

window("Counter", width: 320, height: 200) {
  column(spacing: 12, align: .center, justify: .center) {
    label("Clicked {count} times", style: .title)
    row(spacing: 8) {
      button("−") { count -= 1 }
      button("+", style: .primary) { count += 1 }
    }
  }
}
```

That's the whole program. No setup code, no event listeners, no "refresh the screen"
step. When you click **+**, `count` changes, and the label that shows `count` updates
by itself.

## Why Nifty?

- **It's small.** You can learn the whole language in an afternoon. This book covers
  all of it.
- **It looks native.** Nifty uses your operating system's own widgets, so apps look
  and feel like every other app on your computer, including dark mode and system fonts.
- **The UI updates itself.** You say *what* the screen shows. Nifty works out *when*
  each part needs to change, and updates only those parts.
- **Change it while it runs.** `nf dev` reloads your app every time you save, and
  keeps what it remembered, so you see each change in a moment.
- **Mistakes are caught early.** Nifty checks your program before it runs and explains
  problems in plain words, like
  `` `label` shows text; turn this `int` into text with "{…}" ``.

> **Where Nifty is today:** Nifty is young (version 0). It runs and builds apps on
> **macOS** and **Windows**; Linux comes next. See [What's next](roadmap.md).

## Where to go from here

<div class="cards">
  <a class="card" href="getting-started/install.html"><strong>Install Nifty →</strong><span>Get the <code>nf</code> tool running in a few minutes.</span></a>
  <a class="card" href="tutorials/counter.html"><strong>Build a counter →</strong><span>A step-by-step first app. No experience needed.</span></a>
  <a class="card" href="reference/cheat-sheet.html"><strong>Cheat sheet →</strong><span>The whole language on one page.</span></a>
  <a class="card" href="internals/source-code.html"><strong>Browse the code →</strong><span>Every part of Nifty, with links to the source.</span></a>
</div>

## How this book is organized

| Part | Read it when… |
|---|---|
| **Getting started** | you want Nifty installed and an app on screen. |
| **Tutorials** | you'd like to learn by building something, one step at a time. |
| **The guide** | you want each idea explained properly, with examples. |
| **Reference** | you know what you're looking for and need the exact details. |
| **Under the hood** | you're curious how Nifty works, or want to help build it. |

Code in this book that looks like `this` is something you type or something Nifty
prints. Every page has an edit button (top right) if you spot something to fix.
