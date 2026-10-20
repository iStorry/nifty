# Share your app

`nf run` is great while you work, but other people don't have `nf`. To give them your
app, **build** it:

```bash
nf build todos
```

On macOS you get a `.app`:

```text
built todos/build/todos.app
```

That's a normal macOS app. Double-click it in Finder, drag it into **Applications**, or
send it to a friend.

On Windows you get a folder:

```text
built todos\build\todos
```

Inside is `todos.exe`. Double-click it to run it, or zip the `todos` folder to send it.

## What `nf build` does

1. **Checks your app.** If there are errors, it shows them and builds nothing, just
   like `nf run`.
2. **Compiles it to machine code**, and links that with Nifty's runtime library into
   one program. Your `.nf` files aren't inside the app.
3. **Makes the app** in a `build` folder next to your `main.nf`, named after the
   project folder (`todos` becomes `todos.app` on macOS, or `todos\todos.exe` on
   Windows). If you build a single file such as `demo.nf`, the app is called `demo`.
4. **Replaces any earlier build**, so `build/` always holds the latest version.

Linking uses your system's own linker, which `nf run` doesn't need:

- **macOS:** Xcode's command line tools. Install them with `xcode-select --install`.
- **Windows:** Microsoft's C++ Build Tools. Install them with:

  ```powershell
  winget install Microsoft.VisualStudio.2022.BuildTools --override "--passive --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
  ```

If something goes wrong while the app runs, such as dividing by zero, it stops and
writes the error and where in your code it happened to the terminal it was started from.

> **Tip:** add `build/` to your project's `.gitignore`. It's made from your code, so
> there's no need to keep it in version control.

## Building for other systems

By default `nf build` builds for the system you're on. A flag names the system instead:

```bash
nf build --macos
nf build --windows
nf build --linux
```

`nf` builds apps for the system it runs on, so build macOS apps on a Mac and Windows apps
on Windows. Naming another system explains that. Linux has no backend yet; see
[What's next](../roadmap.md).

## Opening the app on another Mac

Apps from `nf build` aren't signed with an Apple developer certificate, so the first
time someone else opens one, macOS may block it because it can't check who made it.
They can allow it once: after the warning, open **System Settings → Privacy & Security**,
scroll down and click **Open Anyway**. (On older macOS versions, right-click the app
and choose **Open** instead.)

## Opening the app on another Windows PC

Apps from `nf build` aren't signed with a code-signing certificate, so the first time
someone opens one they've downloaded, Windows may show **Windows protected your PC**.
They can click **More info**, then **Run anyway**.
