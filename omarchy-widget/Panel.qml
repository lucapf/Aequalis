import QtQuick
import Quickshell.Io
import Quickshell.Wayland
import qs.Commons
import qs.Ui

// Tally bar widget: a bar button opening a popup with one input field. As you type, the
// request is evaluated by the tally CLI (`tally --json …`), so the widget accepts exactly
// what the command line does: units (`10 km mi`), the calculator (`12+12/3`), dates
// (`2026/12/04 - 12d`, `now unix`). Enter copies the value and keeps the request in Recent; an
// ambiguous date (`12/04/2026 unix`) shows one button per reading.
//
// Settings (this widget's entry in ~/.config/omarchy/shell.json):
//   "command": the tally binary, when it is not on the shell's PATH (default "tally").
// The TUI button opens the full interactive mode (`tally` with no arguments) in a new terminal,
// which gets the focus; it is disabled while that terminal is open. Clicking the title shows
// About: the logo, the tagline, the memory footprint (the last request's peak, the open TUI, the
// binary), the version and the last online sync (`tally --about`). The header shows online
// or offline (`tally --online`), forced offline by TALLY_MODE=offline.
Panel {
  id: root
  moduleName: "lucapf.tally"
  // One IPC target, ours (below): the base's open/close plus `set`, like the built-in clock.
  manageIpc: false

  readonly property string command: setting("command", "tally")

  property string query: ""
  // The parsed `--json` object for `query` (null: nothing typed yet).
  property var reply: null
  // The command could not be run (not installed, or not on PATH).
  property bool missing: false
  // [{input, result}], newest first.
  property var recent: []
  property string copied: ""

  // The TUI's terminal window, found by its app id among the open windows (null: not open).
  readonly property string tuiAppId: "org.omarchy.tally"
  readonly property var tuiWindow: {
    var windows = ToplevelManager.toplevels.values
    for (var i = 0; i < windows.length; i++) {
      if (windows[i] && windows[i].appId === root.tuiAppId) return windows[i]
    }
    return null
  }
  readonly property bool tuiOpen: tuiWindow !== null
  // Set when the TUI was just launched: its window gets the focus once it appears.
  property bool focusTui: false

  // Clicking the title shows tally's memory footprint: the last request's peak (from its `--json`
  // reply), the open TUI's resident memory, and the binary's size.
  property bool showMemory: false
  // From `tally --about` (when About is shown): the logo, the version, the last online sync.
  property string aboutArt: ""
  property string aboutVersion: ""
  property string aboutSync: ""
  // However About is opened (a click, IPC), read it again.
  onShowMemoryChanged: if (root.showMemory) root.refreshAbout()

  // Online or offline (`tally --online`): "checking", "online", "offline" or "offline (forced)"
  // (TALLY_MODE=offline in the shell's environment). Checked when the popup opens and every 30 s.
  property string netStatus: "checking"
  property int tuiKb: -1      // -1: the TUI is not running
  property int binaryKb: -1   // -1: unknown

  onTuiWindowChanged: {
    if (root.tuiWindow && root.focusTui) {
      root.focusTui = false
      focusTimeout.stop()
      root.tuiWindow.activate()
      // Make sure, the way Omarchy focuses a window (Hyprland may ignore an activation request).
      focusProc.command = ["hyprctl", "dispatch", "hl.dsp.focus({ window = \"class:^(" + root.tuiAppId + ")$\" })"]
      focusProc.running = true
    }
  }

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  // The converted value without the format name: `… iso = 2026/11/22 YYYY/MM/DD` → `2026/11/22`;
  // `6.2137 mi` stays as it is (the unit is part of the value).
  // A city's time (`… = 2026-10-11T16:00+02:00 Paris`): the ISO time.
  function valueOf(result) {
    var side = result.indexOf(" = ") >= 0 && /T\d\d:\d\d/.test(result) ? result.slice(result.lastIndexOf(" = ") + 3) : result
    var iso = side.match(/^(\d{4}-\d\d-\d\dT\S+) /)
    if (iso) return iso[1]
    var cut = side.lastIndexOf(" ")
    var last = cut >= 0 ? side.slice(cut + 1) : ""
    var isFormat = /^(iso|iso-[a-z]+|unix|unix-[a-z]+|local|[YMD]{2,4}\/[YMD]{2,4}\/[YMD]{2,4}(-(12|24)h(-tz)?)?)$/.test(last)
    return isFormat ? side.slice(0, cut) : side
  }

  function evaluate() {
    var words = root.query.trim().split(/\s+/).filter(function(w) { return w !== "" })
    if (words.length === 0) {
      root.reply = null
      return
    }
    if (proc.running) {
      proc.pending = true
      return
    }
    proc.command = [root.command, "--json"].concat(words)
    proc.running = true
  }

  // Enter: copy the value and remember the request.
  function commit() {
    if (!root.reply || !root.reply.ok) return
    var entry = { input: root.query.trim(), result: root.reply.result }
    root.recent = [entry].concat(root.recent.filter(function(e) { return e.input !== entry.input })).slice(0, 6)
    root.copied = root.valueOf(root.reply.result)
    copyProc.command = ["wl-copy", "--", root.copied]
    copyProc.running = true
  }

  // The full interactive mode in a new terminal window, which gets the focus. Only one at a time:
  // the button is disabled while it is open.
  function openTui() {
    if (root.tuiOpen) return
    root.focusTui = true
    focusTimeout.restart()
    tuiProc.command = ["omarchy-launch-tui", "--app-id=" + root.tuiAppId, root.command]
    tuiProc.running = true
    root.close()
  }

  // 900 KiB, 2.9 MiB.
  function size(kb) {
    return kb >= 1024 ? (kb / 1024).toFixed(1) + " MiB" : kb + " KiB"
  }

  function memoryLine() {
    var parts = []
    if (root.reply && root.reply.peak_kb) parts.push("last request " + root.size(root.reply.peak_kb) + " peak")
    else parts.push("last request: type one")
    parts.push(root.tuiKb >= 0 ? "TUI " + root.size(root.tuiKb) : "TUI not open")
    if (root.binaryKb >= 0) parts.push("binary " + root.size(root.binaryKb))
    return "Memory · " + parts.join(" · ")
  }

  function refreshAbout() {
    if (!aboutProc.running) aboutProc.running = true
  }

  function refreshMemory() {
    if (!memProc.running) memProc.running = true
  }

  function use(text) {
    field.text = text
    field.cursorPosition = text.length
    field.forceActiveFocus()
  }

  Process {
    id: proc
    property bool pending: false
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var line = String(text || "").trim().split("\n")[0]
        root.missing = line === ""
        if (line === "") return
        try {
          root.reply = JSON.parse(line)
        } catch (e) {
          root.reply = { ok: false, error: "unexpected output: " + line }
        }
      }
    }
    onExited: function(exitCode) {
      if (proc.pending) {
        proc.pending = false
        Qt.callLater(root.evaluate)
      }
    }
  }

  Process { id: copyProc }

  // The binary's size, and the resident memory of the TUI: the `tally` process with no arguments.
  Process {
    id: memProc
    command: ["sh", "-c",
      "bin=$(command -v \"$1\") || exit 0; echo \"binary $(( $(stat -Lc %s \"$bin\") / 1024 ))\"; " +
      "ps -C \"$(basename \"$bin\")\" -o rss=,args= | awk 'NF == 2 { print \"tui\", $1; exit }'",
      "sh", root.command]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var tui = -1, binary = -1
        String(text || "").split("\n").forEach(function(line) {
          var f = line.trim().split(/\s+/)
          if (f[0] === "tui") tui = parseInt(f[1])
          if (f[0] === "binary") binary = parseInt(f[1])
        })
        root.tuiKb = tui
        root.binaryKb = binary
      }
    }
  }

  Process {
    id: netProc
    command: [root.command, "--online"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var word = String(text || "").trim()
        root.netStatus = word === "" ? "offline" : word
      }
    }
  }

  Timer {
    interval: 30000
    repeat: true
    running: root.opened
    triggeredOnStart: true
    onTriggered: if (!netProc.running) netProc.running = true
  }

  // Keep the memory line current while it is shown.
  Timer {
    interval: 2000
    repeat: true
    running: root.showMemory && root.opened
    triggeredOnStart: true
    onTriggered: root.refreshMemory()
  }

  // `tally --about`: the logo (the lines before the first blank one), then `Label: value` lines.
  Process {
    id: aboutProc
    command: [root.command, "--about"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var out = String(text || "")
        var blank = out.indexOf("\n\n")
        root.aboutArt = blank > 0 ? out.slice(0, blank) : ""
        out.split("\n").forEach(function(line) {
          if (line.indexOf("Version: ") === 0) root.aboutVersion = line.slice(9)
          if (line.indexOf("Last online sync: ") === 0) root.aboutSync = line.slice(18)
        })
      }
    }
  }

  Process { id: tuiProc }

  Process { id: focusProc }

  // Stop waiting for the TUI's window if it never shows up (the terminal failed to start).
  Timer {
    id: focusTimeout
    interval: 10000
    onTriggered: root.focusTui = false
  }

  // From scripts and key bindings:
  //   omarchy-shell lucapf.tally toggle
  //   omarchy-shell lucapf.tally set "10 km mi"      (open with a request filled in)
  //   omarchy-shell lucapf.tally set "$(wl-paste)"
  IpcHandler {
    target: "lucapf.tally"
    function open(): void { root.open() }
    function close(): void { root.close() }
    function show(): void { root.open() }
    function hide(): void { root.close() }
    function toggle(): void { root.toggle() }
    function set(text: string): void {
      root.open()
      root.use(text)
    }
    // The TUI button: open the TUI in a new terminal (nothing when it is already open).
    function tui(): void { root.openTui() }
    // The Copy button (Enter does the same): copy the value, keep the request in Recent.
    function copy(): void { root.commit() }
    // Clicking the title: show or hide About.
    function memory(): void { root.showMemory = !root.showMemory }
    function about(): void { root.showMemory = !root.showMemory }
  }

  // Wait for a short pause in typing before running tally.
  Timer {
    id: debounce
    interval: 120
    repeat: false
    onTriggered: root.evaluate()
  }

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: ""  // Nerd Font: calculator
    tooltipText: "Tally"
    onPressed: function(b) { root.toggle() }
  }

  KeyboardPanel {
    id: panel
    anchorItem: button
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: field
    contentWidth: panel.fittedContentWidth(Style.space(480))
    contentHeight: panel.fittedContentHeight(column.implicitHeight)

    Column {
      id: column
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: parent.top
      spacing: Style.space(10)

      // Title, a small button opening the full TUI in the terminal, and one closing the popup.
      Item {
        width: parent.width
        implicitHeight: Math.max(title.implicitHeight, headerButtons.implicitHeight)

        Text {
          id: title
          anchors.left: parent.left
          anchors.verticalCenter: parent.verticalCenter
          text: "Tally"
          color: titleMouse.containsMouse ? Color.accent : Color.foreground
          font.family: Style.font.family
          font.pixelSize: Style.font.subtitle
          font.bold: true

          MouseArea {
            id: titleMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: root.showMemory = !root.showMemory
          }
        }

        Row {
          id: headerButtons
          anchors.right: parent.right
          anchors.verticalCenter: parent.verticalCenter
          spacing: Style.space(4)

          // Online / offline, before the buttons.
          Text {
            id: netIndicator
            anchors.verticalCenter: parent.verticalCenter
            rightPadding: Style.space(6)
            text: (root.netStatus === "online" ? "● " : root.netStatus === "checking" ? "○ " : "● ") + root.netStatus
            color: root.netStatus === "online" ? Color.accent : Color.muted
            font.family: Style.font.family
            font.pixelSize: Style.font.caption

            MouseArea {
              id: netMouse
              anchors.fill: parent
              hoverEnabled: true
            }

            PanelToolTip {
              visible: netMouse.containsMouse
              text: root.netStatus === "offline (forced)" ? "TALLY_MODE=offline" : "Can JPL Horizons and CelesTrak be reached (the Trivia's live values)"
            }
          }

          Button {
            id: tuiButton
            iconText: "\uf120"  // Nerd Font: terminal
            text: "TUI"
            fontSize: Style.font.caption
            enabled: !root.tuiOpen
            opacity: enabled ? 1 : 0.4
            tooltipText: root.tuiOpen ? "The tally TUI is already open" : "Open the tally TUI in a new terminal"
            onClicked: root.openTui()
          }
        }
      }

      // About (click the title to show or hide it): the logo, the tagline, the memory footprint,
      // the version and the last online sync.
      Column {
        width: parent.width
        visible: root.showMemory
        spacing: Style.space(4)

        Text {
          visible: root.aboutArt !== ""
          textFormat: Text.PlainText
          color: Color.accent
          font.family: "monospace"
          font.pixelSize: Style.font.caption
          text: root.aboutArt
        }

        Text {
          width: parent.width
          wrapMode: Text.Wrap
          textFormat: Text.PlainText
          color: Color.foreground
          font.family: Style.font.family
          font.pixelSize: Style.font.body
          font.bold: true
          text: "Tally — Ultralight calculator and converter made for humans"
        }

        Text {
          width: parent.width
          wrapMode: Text.Wrap
          textFormat: Text.PlainText
          color: Color.muted
          font.family: Style.font.family
          font.pixelSize: Style.font.caption
          text: root.memoryLine()
            + (root.aboutVersion !== "" ? "\nVersion " + root.aboutVersion : "")
            + "\nLast online sync: " + (root.aboutSync !== "" ? root.aboutSync : "…")
        }
      }

      TextField {
        id: field
        width: parent.width
        placeholderText: "10 km mi  ·  12+12/3  ·  2026/12/04 - 12d  ·  now unix"
        onTextChanged: {
          root.query = text
          root.copied = ""
          debounce.restart()
        }
        onAccepted: root.commit()
        Keys.onEscapePressed: root.close()
      }

      // The result, or why there is none; a Copy button beside a result.
      Item {
        width: parent.width
        visible: resultText.text !== ""
        implicitHeight: Math.max(resultText.implicitHeight, copyButton.visible ? copyButton.implicitHeight : 0)

        Text {
          id: resultText
          anchors.left: parent.left
          anchors.right: copyButton.visible ? copyButton.left : parent.right
          anchors.rightMargin: copyButton.visible ? Style.space(8) : 0
          anchors.verticalCenter: parent.verticalCenter
          wrapMode: Text.WrapAtWordBoundaryOrAnywhere
          textFormat: Text.PlainText
          color: root.missing || (root.reply && !root.reply.ok) ? Color.muted : Color.foreground
          font.family: Style.font.family
          font.pixelSize: root.reply && root.reply.ok ? Style.font.subtitle : Style.font.body
          text: root.missing
            ? "`" + root.command + "` not found: install tally (see docs/omarchy-widget.md)"
            : !root.reply ? ""
            : root.reply.ok ? "= " + root.reply.result
            : root.reply.ambiguous && root.reply.ambiguous.kind === "city" ? (root.reply.ambiguous.note || "`" + root.reply.ambiguous.text + "` is not a city") + ": did you mean…"
            : root.reply.ambiguous && root.reply.ambiguous.kind === "country" ? "`" + root.reply.ambiguous.text + "` has several time zones: which one?"
            : root.reply.ambiguous && root.reply.ambiguous.kind === "currency" ? root.reply.ambiguous.note + ": which one?"
            : root.reply.ambiguous ? "Which date is " + root.reply.ambiguous.text + "?"
            : root.reply.error
        }

        Button {
          id: copyButton
          anchors.right: parent.right
          anchors.verticalCenter: parent.verticalCenter
          visible: !root.missing && root.reply !== null && root.reply.ok === true
          iconText: "\uf0c5"  // Nerd Font: copy
          text: "Copy"
          fontSize: Style.font.caption
          tooltipText: visible ? "Copy " + root.valueOf(root.reply.result) + " to the clipboard" : ""
          onClicked: root.commit()
        }
      }

      // Notes (how a date was read) and warnings.
      Text {
        width: parent.width
        visible: text !== ""
        wrapMode: Text.Wrap
        textFormat: Text.PlainText
        color: Color.muted
        font.family: Style.font.family
        font.pixelSize: Style.font.caption
        text: root.reply && root.reply.ok ? (root.reply.warnings || []).concat(root.reply.notes || []).join("\n") : ""
      }

      // An ambiguous date: one button per reading; the chosen pattern is written in. A misspelt
      // city: one button per city it may be. A country with several time zones: one per zone, its
      // most relevant city first, then its offset now. A shared or misspelt currency (`dollar`,
      // `dolar`): one per currency it may be, its name and code.
      Flow {
        width: parent.width
        spacing: Style.space(6)
        visible: root.reply !== null && root.reply.ambiguous !== undefined
        Repeater {
          model: root.reply && root.reply.ambiguous ? root.reply.ambiguous.choices : []
          Button {
            bordered: true
            text: modelData.pattern + "   " + modelData.date
            onClicked: root.use(modelData.request)
          }
        }
      }

      Text {
        visible: root.copied !== ""
        text: "Copied " + root.copied
        color: Color.accent
        font.family: Style.font.family
        font.pixelSize: Style.font.caption
      }

      Text {
        visible: root.recent.length > 0
        text: "Recent"
        color: Color.muted
        font.family: Style.font.family
        font.pixelSize: Style.font.caption
      }

      Repeater {
        model: root.recent
        Button {
          width: column.width
          leftAlign: true
          text: modelData.input + "   =   " + modelData.result
          onClicked: root.use(modelData.input)
        }
      }

      Text {
        text: "Enter or Copy copies · Esc closes · click Tally for About"
        color: Color.muted
        font.family: Style.font.family
        font.pixelSize: Style.font.caption
      }
    }
  }
}
