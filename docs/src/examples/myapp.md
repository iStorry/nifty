# Split across files

A project whose UI lives in a second file. `main.nf` uses the `Header` view, which is
declared in `ui/abc.nf`. There is no import: every `.nf` file in the project folder is
part of the app, and they share one set of names.

```text
myapp/
  main.nf
  ui/
    abc.nf
```

`main.nf`:

```nf
{{#include ../../../examples/myapp/main.nf}}
```

`ui/abc.nf`:

```nf
{{#include ../../../examples/myapp/ui/abc.nf}}
```

**Run it:**

```bash
nf run examples/myapp
```

**Source:** [`examples/myapp`](https://github.com/istorry/nifty/tree/main/examples/myapp)

A name declared in two files is an error that points at both, and `nf dev` reloads when
you save any of the files. See [Projects](../reference/language.md#projects-and-the-nf-tool)
for the rules.
