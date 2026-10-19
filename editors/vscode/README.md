# Nifty for VS Code

Syntax highlighting for `.nf` files, plus diagnostics, semantic highlighting, the outline,
go to definition, find references and highlighting the name under the cursor, all served
by `nf lsp`.

## Develop

Build `nf` and put it on `PATH`, or set `nifty.server.path` to the binary:

```bash
cargo build -p nifty-cli
```

Then, from this folder:

```bash
npm install
npm run compile
npm test
```

Open this folder in VS Code and press F5 to start an Extension Development Host with the
extension loaded. After rebuilding `nf`, run **Nifty: Restart Language Server**.

`npm test` checks the TextMate grammar against the assertions in `tests/`.
