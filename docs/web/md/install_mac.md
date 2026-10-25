# Install Aequalis on macOS

macOS 11 or later, Apple Silicon or Intel.

## Install with Homebrew

```
brew install --cask lucapf/tap/aequalis
```

It installs **Aequalis.app** (in Applications) and the `aequalis` command. Open Aequalis once: its icon
appears in the menu bar.

## Install without Homebrew

1. Download `aequalis-<version>-macos.zip` from the
   [latest release](https://github.com/lucapf/aequalis/releases/latest), unzip it and drag
   **Aequalis.app** to Applications.
2. The app is not signed with an Apple developer account yet, so macOS refuses it the first time.
   Either right-click it › **Open** › **Open**, or in the terminal:

   ```
   xattr -dr com.apple.quarantine /Applications/Aequalis.app
   ```
3. For the `aequalis` command:

   ```
   ln -s /Applications/Aequalis.app/Contents/Resources/aequalis /usr/local/bin/aequalis
   ```

## Use it

- Press **⌥⌘A** anywhere, or **click the icon** in the menu bar. The input box opens; type a request
  and the answer appears as you type.
- **Enter** copies the result and closes the box. **Esc**, or a click anywhere else, closes it.
- When a request can be read in more than one way (`12/04/2026 unix`), a button appears for each
  reading; **Enter** takes the first one.
- **Right-click the icon** for its menu: Download data, Settings, Open at login, About, Log, Quit.

### Select text anywhere

Select `12 usd eur` in any app, then **Services** (the app's menu, or right-click) ›
**Aequalis: replace with the answer** or **Aequalis: copy the answer**. To give them a key, go to
**System Settings › Keyboard › Keyboard Shortcuts › Services › Text**.

## The command line

```console
$ aequalis 10 km mi
6.2137 mi
$ aequalis
```

`aequalis` with no arguments opens the interactive mode in the terminal.

## Settings and data

In `~/.config/aequalis`, shared by the app and the command line. **Settings** in the icon's menu opens
the file; `aequalis --settings` lists them all, and `aequalis --set key=value` changes one.

## If something doesn't work

The app writes what it does to `~/Library/Logs/Aequalis.log` (**Log** in the icon's menu). If ⌥⌘A does
nothing, the log says whether another app has that key; the icon still works.

## Uninstall

`brew uninstall --cask aequalis`, or drag Aequalis.app to the Bin. Your settings and data stay in
`~/.config/aequalis`.

## Build it yourself

With [Rust](https://rustup.rs) and the Xcode command line tools:

```
apps/macos/build.sh 0.1.0
```

The app and its zip are written to `dist/`.
