# Install Tally on Mac os

Three ways to use Tally on a Mac (Apple Silicon): the command line, the menu bar app, and
Raycast. The menu bar app has the command line inside it.

## Command line

1. open the terminal
2. execute the following script:
```
curl -LO https://github.com/lucapf/tally/releases/download/v0.1.1/tally-0.1.1-aarch64-macos.tar.gz
tar xzf tally-0.1.1-aarch64-macos.tar.gz
[[ ! -d ~/.local/bin ]] && mkdir -p ~/.local/bin
cp tally-0.1.1-aarch64-macos/tally ~/.local/bin/
```
## Untrusted sources

NOTE:
if you download the file using the browser insetead, your mac will refuse to execute the  Tally because the executable is not signed.

To avoid this issue:
```
xattr -d com.apple.quarantine ~/.local/bin/tally
```

## Test tally
`tally 10 km mi`

## Menu bar app

Tally in the menu bar: click its icon (a tally mark) or press **⌃⌥T** anywhere, type a request,
and Enter copies the answer. It has the same features as the Omarchy widget: the choices for an
ambiguous request, Recent, online/offline, the TUI button (opens Terminal) and About (click
"Tally"), where you can also make it open at login, turn the network off, or quit it.
It needs macOS 13 or later.

1. Download `Tally-<version>-macos.zip` from the [releases](https://github.com/lucapf/tally/releases)
   and unzip it.
2. Move `Tally.app` to `Applications`.
3. The first time, macOS refuses to open it, because the app is not signed by an Apple developer:
   open **System Settings → Privacy & Security** and click **Open Anyway** next to Tally. Or, in the
   terminal:
   ```
   xattr -dr com.apple.quarantine /Applications/Tally.app
   ```

## Raycast

The extension is in `raycast/` (not yet in the Raycast Store). It runs the `tally` command, so
install the command line first (above). Then, from the repository:
```
cd raycast
npm install
npm run dev
```
Raycast opens with **Calculate with Tally** in its root search.

