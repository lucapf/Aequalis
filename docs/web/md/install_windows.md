# Install Aequalis on Windows

Windows 10 (version 1903 or later) or Windows 11, 64-bit.

## Install

1. Download `aequalis-<version>-x86_64-windows-setup.exe` from the
   [latest release](https://github.com/lucapf/aequalis/releases/latest).
2. Run it. It installs for your user only, so it asks for no administrator password.
3. Keep both options ticked, or untick what you don't want:
   - **Start Aequalis when I sign in**: its icon stays in the notification area.
   - **Add the aequalis command to the PATH**: `aequalis 10 km mi` works in any terminal.

> **"Windows protected your PC"**: the installer is not signed yet, so SmartScreen warns about it.
> Click **More info**, then **Run anyway**.

## Use it

- **Click the Aequalis icon** in the notification area (by the clock), or press **Win+Alt+A**
  anywhere. The input box opens; type a request and the answer appears as you type.
- **Enter** copies the result. **Esc**, or a click anywhere else, closes the box.
- When a request can be read in more than one way (`12/04/2026 unix`), a button appears for each
  reading; **Enter** takes the first one.
- **Right-click the icon** for its menu: download the online data now, the settings, start with
  Windows (on or off), About, Quit.
- Click **Aequalis**, at the top of the box, for About: the version, memory, the online data.

Windows 11 may put a new icon under the **^** arrow by the clock. To keep it always visible: right-click
the taskbar › **Taskbar settings** › **Other system tray icons**, and turn **Aequalis** on.

The time zone is always the computer's (read again every hour, and at once when it changes), and the
zones' rules are those of Windows itself, kept up to date by Windows Update.

## The command line

`aequalis` works in PowerShell, Windows Terminal and the Command Prompt, as on Linux and macOS:

```console
> aequalis 10 km mi
6.2137 mi
> aequalis 10:00AM Paris to New York
18 Oct 26 4:00 AM GMT-4 New York
```

## The interactive mode

`aequalis` with no arguments opens the interactive mode in the console, as on Linux and macOS: type
a request and the answer shows as you type. **F2** Measures, **F3** Currencies, **F4** Sizes,
**F5** the history, **F6** Trivia, **F9** the keypad (Basic, Sci, hidden), **F10** Settings, **F1** all the keys, **Ctrl+K** searches units and
currencies, **Ctrl+Q** quits. It needs Windows 10 version 1903 or later, in **Windows Terminal**,
PowerShell or the Command Prompt. If a key is taken by your terminal, **Alt+M**, **Alt+C**,
**Alt+T**, **Alt+H** and **Alt+S** open the same windows. **Alt+Y** copies the result to the
clipboard (through PowerShell, so it takes a moment the first time).

## Settings and data

The settings and the downloaded data (exchange rates, the city archive) are in `%APPDATA%\aequalis`,
shared by the input box and the command line. **Settings…** in the icon's menu opens the settings file;
`aequalis --settings` lists them all, and `aequalis --set key=value` changes one.

## Uninstall

**Settings › Apps › Installed apps › Aequalis › Uninstall.** Your settings and data stay in
`%APPDATA%\aequalis`; delete that folder to remove them too.

## Build it yourself

With [Rust](https://rustup.rs), the Visual Studio Build Tools (C++) and
[Inno Setup 6](https://jrsoftware.org/isinfo.php):

```powershell
cargo build --release --workspace
& "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe" installer\windows\aequalis.iss
```

The installer is written to `dist\`.
