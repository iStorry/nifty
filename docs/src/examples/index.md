# Examples

Complete, working Nifty apps. Each one lives in the
[`examples` folder](https://github.com/istorry/nifty/tree/main/examples) of the
repository, and the code on these pages is pulled straight from those files, so it's
always the real thing.

| Example | Shows you |
|---|---|
| [Todos](todos.md) | structs, lists, functions, views, text fields, checkboxes |
| [Downloader](downloader.md) | background work with `download`, binding to list items |
| [Gallery](gallery.md) | every element and every style, side by side |

Looking for the counter? It's the starter app: `nf create my-app` gives you one, and the
[counter tutorial](../tutorials/counter.md) builds it step by step.

To run an example, clone the repository and point `nf run` at its folder:

```bash
nf run examples/todos
```

Or use `nf dev` to play with it: change the code, save, and the window updates
straight away.

```bash
nf dev examples/gallery
```
