# Set up your editor

Nifty files end in `.nf`. Any text editor can open them, but with editor support you
get colors, errors as you type, and jumping to definitions.

All editor features come from **`nf lsp`**, a *language server* built into `nf`.
Editors start it for you; you never run it by hand. So first make sure `nf` is
installed and on your `PATH` ([Install Nifty](install.md)).

| Feature | VS Code | Zed |
|---|:---:|:---:|
| Syntax colors | ✓ | ✓ |
| Errors as you type | ✓ | ✓ |
| Autocomplete | ✓ | ✓ |
| Signature help and hover docs | ✓ | ✓ |
| Outline of your file | ✓ | ✓ |
| Go to definition | ✓ | ✓ |
| Find all references | ✓ | ✓ |
| Highlight the name under the cursor | ✓ | ✓ |
| Auto-indent, brackets, text objects | | ✓ |

## VS Code

The extension lives in
[`editors/vscode`](https://github.com/istorry/nifty/tree/main/editors/vscode). It
isn't on the Marketplace yet, so you run it from the source:

1. Build it once:

   ```bash
   cd editors/vscode
   npm install
   npm run compile
   ```

2. Open the `editors/vscode` folder in VS Code and press **F5**. A second VS Code window
   opens with Nifty support turned on. Open any `.nf` file there.

If `nf` isn't on your `PATH`, set **`nifty.server.path`** in VS Code's settings to the
full path of the `nf` program.

After you rebuild `nf`, run the command **Nifty: Restart Language Server** from the
command palette (**⇧⌘P** on macOS, **Ctrl+Shift+P** on Windows).

## Zed

The extension lives in
[`editors/zed`](https://github.com/istorry/nifty/tree/main/editors/zed).

1. In Zed, open the command palette (**⇧⌘P** on macOS, **Ctrl+Shift+P** on Windows) and
   run **zed: install dev extension**.
2. Pick the `editors/zed` folder.

If `nf` isn't on your `PATH`, tell Zed where it is in `settings.json`:

```json
{
  "lsp": {
    "nifty": {
      "binary": { "path": "/path/to/nf" }
    }
  }
}
```

After you rebuild `nf`, run **editor: restart language server**.

## Other editors

Any editor that speaks the
[Language Server Protocol](https://microsoft.github.io/language-server-protocol/) can
use Nifty. Configure it to start the command `nf lsp` for files ending in `.nf`.

For syntax colors, there's a [Tree-sitter grammar](https://github.com/istorry/nifty/tree/main/editors/tree-sitter-nifty)
(used by Zed, Neovim, Helix and others) and a
[TextMate grammar](https://github.com/istorry/nifty/blob/main/editors/vscode/syntaxes/nifty.tmLanguage.json)
(used by VS Code, Sublime Text and others).
