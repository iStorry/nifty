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

Inside is `todos.exe`, with your program in an `app` folder next to it. Keep the two
together: zip the whole `todos` folder to send it, and double-click `todos.exe` to run it.

## What `nf build` does

1. **Checks your app.** If there are errors, it shows them and builds nothing, just
   like `nf run`.
2. **Makes the app** in a `build` folder next to your `main.nf`, named after the
   project folder (`todos` becomes `todos.app` on macOS, or `todos\todos.exe` on
   Windows). If you build a single file such as `demo.nf`, the app is called `demo`.
3. **Replaces any earlier build**, so `build/` always holds the latest version.

Inside, the app holds your project's `.nf` files and a small program called `nf-runner` that runs
it. `cargo install` puts `nf-runner` next to `nf` for you; if `nf build` says it's
missing, install again:

```bash
cargo install --path crates/nifty-cli
```

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
