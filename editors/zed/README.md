# Nifty for Zed

Syntax highlighting, brackets, indentation, the outline and text objects for `.nf`
files, from the Tree-sitter grammar in [`../tree-sitter-nifty`](../tree-sitter-nifty).
Diagnostics, completion, signature help, hover, semantic highlighting, go to definition,
find references and highlighting the name under the cursor come from `nf lsp`.

## Develop

Build `nf` and put it on `PATH`:

```bash
cargo build -p nifty-cli
```

Zed clones the grammar from GitHub, as `[grammars.nifty]` in `extension.toml` says, at
the pinned `rev`. So commit and push grammar changes, then update `rev` to that commit
before reinstalling.

In Zed, run **zed: install dev extension** and pick this folder. After rebuilding `nf`,
run **editor: restart language server**.

To use an `nf` that is not on `PATH`, set it in Zed's `settings.json`:

```json
{
  "lsp": {
    "nifty": {
      "binary": { "path": "/path/to/nf" }
    }
  }
}
```

Check the extension compiles with:

```bash
cargo build --release --target wasm32-wasip2
```
