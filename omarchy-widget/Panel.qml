import QtQuick
import Quickshell.Io
import Quickshell.Wayland
import qs.Commons
import qs.Ui

// Aequalis bar widget: a bar button opening a popup with one input field. As you type, the
// request is evaluated by the aequalis CLI as typed (`aequalis --json --as-typed "…"`), as the
// Android app does: units (`10 km mi`), the calculator (`12+12/3`), dates (`2026/12/04 - 12d`,
// `now unix`). Enter copies the value and keeps the request in Recent; an ambiguous date
// (`12/04/2026 unix`) shows one button per reading. What shows, aequalis's reply says (`show`,
// `message`, `copy`, `choices`): while a word is being typed (no space after it yet, no Enter)
// errors and questions wait, only a result shows.
//
// Settings (this widget's entry in ~/.config/omarchy/shell.json):
//   "command": the aequalis binary, when it is not on the shell's PATH (default "aequalis").
// The TUI button opens the full interactive mode (`aequalis` with no arguments) in a new terminal,
// which gets the focus; it is disabled while that terminal is open. Clicking the title shows
// About: the logo, the tagline, the memory footprint (the last request's peak, the open TUI, the
// binary), the version and the last online sync (`aequalis --about`). The header shows online
// or offline (`aequalis --online`), forced by the offline setting (`aequalis --set offline=true`).
// Notices (a new city archive, out-of-date data, from `--json`) come with a Download button.
Panel {
  id: root
  moduleName: "lucapf.aequalis"
  // One IPC target, ours (below): the base's open/close plus `set`, like the built-in clock.
  manageIpc: false

  readonly property string command: setting("command", "aequalis")

  property string query: ""
  // The parsed `--json` object for `query` (null: nothing typed yet).
  property var reply: null
  // Enter was pressed on this query: aequalis then shows its error even in the middle of a word
  // (what shows, the reply's `show`, aequalis decides: `--as-typed`).
  property bool submitted: false
  // The command could not be run (not installed, or not on PATH).
  property bool missing: false
  // [{input, result}], newest first.
  property var recent: []
  property string copied: ""

  // The TUI's terminal window, found by its app id among the open windows (null: not open).
  readonly property string tuiAppId: "org.omarchy.aequalis"
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

  // Clicking the title shows aequalis's memory footprint: the last request's peak (from its `--json`
  // reply), the open TUI's resident memory, and the binary's size.
  property bool showMemory: false
  // From `aequalis --about` (when About is shown): the logo, the version, the last online sync.
  property string aboutArt: ""
  property string aboutVersion: ""
  property string aboutSync: ""
  // However About is opened (a click, IPC), read it again.
  onShowMemoryChanged: if (root.showMemory) root.refreshAbout()

  // Online or offline (`aequalis --online`): "checking", "online", "offline" or "offline (forced)"
  // (the offline setting). Checked when the popup opens and every 30 s.
  property string netStatus: "checking"
  // Notices from `aequalis --online --json` (the city archive not downloaded, out-of-date data), each
  // with a Download button; refreshed with the online state and after a download.
  property var notices: []
  // The Settings section (the gear): every setting from `aequalis --settings --json`, changed with
  // `aequalis --set`; the message is what the last change or download said.
  property bool showSettings: false
  property var settingsList: []
  property string settingsMessage: ""
  onShowSettingsChanged: if (root.showSettings) { root.showMemory = false; root.refreshSettings() }
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

  // The query as typed (a trailing space says a word is done), and whether Enter was pressed on it:
  // the reply says what to show.
  function evaluate() {
    if (root.query.trim() === "") {
      root.reply = null
      return
    }
    if (proc.running) {
      proc.pending = true
      return
    }
    proc.command = [root.command, "--json", "--as-typed"].concat(root.submitted ? ["--submitted"] : []).concat([root.query])
    proc.running = true
  }

  // Enter: copy the value and remember the request; on anything else, the reply as submitted (an
  // error or a question shows even in the middle of a word).
  function commit() {
    if (!root.submitted) {
      root.submitted = true
      if (!root.reply || root.reply.show !== "result") root.evaluate()
    }
    if (!root.reply || root.reply.show !== "result") return
    var entry = { input: root.query.trim(), result: root.reply.result }
    root.recent = [entry].concat(root.recent.filter(function(e) { return e.input !== entry.input })).slice(0, 6)
    root.copied = root.reply.copy
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

  function refreshSettings() {
    if (!settingsProc.running) settingsProc.running = true
  }

  // A setting changed (`null`: back to its default).
  function setValue(key, value) {
    setProc.command = value === null ? [root.command, "--set", key] : [root.command, "--set", key + "=" + value]
    setProc.running = true
  }

  // A yes/no or a choice: the next value.
  function nextValue(s) {
    if (s.kind === "bool") return s.value === "true" ? "false" : "true"
    var i = s.options.indexOf(s.value)
    return s.options[(i + 1) % s.options.length]
  }

  // `aequalis --sync` (everything) in the background; the notices follow when it ends.
  function download() {
    if (syncProc.running) return
    root.settingsMessage = "Downloading everything…"
    syncProc.command = [root.command, "--sync"]
    syncProc.running = true
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
          root.reply = { ok: false, show: "error", message: "unexpected output: " + line }
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
  // A download asked for from a notice or the Settings (`aequalis --sync`).
  Process {
    id: syncProc
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: if (String(text || "").trim() !== "") root.settingsMessage = String(text).trim()
    }
    stderr: StdioCollector {
      waitForEnd: true
      onStreamFinished: if (String(text || "").trim() !== "") root.settingsMessage = String(text).trim()
    }
    onExited: { if (!netProc.running) netProc.running = true }
  }

  Process {
    id: settingsProc
    command: [root.command, "--settings", "--json"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        try { root.settingsList = JSON.parse(String(text || "{}")).settings || [] } catch (e) { root.settingsList = [] }
      }
    }
  }

  Process {
    id: setProc
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: if (String(text || "").trim() !== "") root.settingsMessage = "Saved: " + String(text).trim()
    }
    stderr: StdioCollector {
      waitForEnd: true
      onStreamFinished: if (String(text || "").trim() !== "") root.settingsMessage = String(text).trim().replace(/^aequalis: error: /, "")
    }
    onExited: { root.refreshSettings(); if (!netProc.running) netProc.running = true }
  }

  // The binary's size, and the resident memory of the TUI: the `aequalis` process with no arguments.
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
    command: [root.command, "--online", "--json"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var reply = null
        try { reply = JSON.parse(String(text || "")) } catch (e) {}
        root.netStatus = reply && reply.result ? reply.result : "offline"
        root.notices = reply && reply.notices ? reply.notices : []
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

  // `aequalis --about`: the logo (the lines before the first blank one), then `Label: value` lines.
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
  //   omarchy-shell lucapf.aequalis toggle
  //   omarchy-shell lucapf.aequalis set "10 km mi"      (open with a request filled in)
  //   omarchy-shell lucapf.aequalis set "$(wl-paste)"
  IpcHandler {
    target: "lucapf.aequalis"
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
    function settings(): void { root.showSettings = !root.showSettings }
  }

  // Wait for a short pause in typing before running aequalis.
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
    // The icon's mark (iconset/icons/master/aequalis-icon-small.svg: the q and, in its bowl, the
    // bolder arrows of the small sizes) in the bar's colour, its 608×580 box scaled to the slot.
    iconComponent: Component {
      Item {
        Canvas {
          readonly property real size: Style.space(16)
          readonly property color color: root.barForeground
          anchors.centerIn: parent
          width: size
          height: Math.ceil(size * 580 / 608)
          onColorChanged: requestPaint()
          onSizeChanged: requestPaint()
          onPaint: {
            var ctx = getContext("2d")
            ctx.reset()
            ctx.scale(size / 608, size / 608)
            ctx.translate(-208, -220)
            ctx.fillStyle = color
            ctx.fillRule = Qt.OddEvenFill
            ctx.path = "M 468 220 A 260 260 0 0 1 716 402 L 716 220 L 816 220 L 816 750 A 50 50 0 0 1 716 750 L 716 558 A 260 260 0 0 1 468 740 A 260 260 0 0 1 468 220 Z M 468 330 A 150 150 0 0 1 468 630 A 150 150 0 0 1 468 330 Z"
            ctx.fill()
            ctx.fillRule = Qt.WindingFill
            ctx.path = "M 356 388 L 504 388 L 504 358 Q 504 338 522 348 L 598 428 Q 614 438 598 448 L 522 502 Q 504 512 504 492 L 504 464 L 356 464 A 38 38 0 0 1 356 388 Z M 580 572 L 432 572 L 432 544 Q 432 524 414 534 L 338 614 Q 322 624 338 634 L 414 688 Q 432 698 432 678 L 432 650 L 580 650 A 38 38 0 0 0 580 572 Z"
            ctx.fill()
          }
        }
      }
    }
    tooltipText: "Aequalis"
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
          text: "Aequalis"
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
              text: root.netStatus === "offline (forced)" ? "The offline setting (aequalis --set offline=false)" : "The last download of the online data"
            }
          }

          // The gear: the Settings section.
          Button {
            id: settingsButton
            iconText: "\uf013"  // Nerd Font: gear
            text: ""
            fontSize: Style.font.caption
            tooltipText: root.showSettings ? "Back to the calculator" : "Settings"
            onClicked: root.showSettings = !root.showSettings
          }

          Button {
            id: tuiButton
            iconText: "\uf120"  // Nerd Font: terminal
            text: "TUI"
            fontSize: Style.font.caption
            enabled: !root.tuiOpen
            opacity: enabled ? 1 : 0.4
            tooltipText: root.tuiOpen ? "The aequalis TUI is already open" : "Open the aequalis TUI in a new terminal"
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
          text: "Aequalis — Ultralight calculator and converter made for humans"
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

      // Settings (the gear): each setting with its value; a yes/no or a choice changes with a click,
      // the others are typed (Enter saves); "default" puts back the default. Then the downloads.
      Column {
        width: parent.width
        visible: root.showSettings
        spacing: Style.space(4)

        Text {
          text: "\uf013  Settings"
          color: Color.foreground
          font.family: Style.font.family
          font.pixelSize: Style.font.body
          font.bold: true
        }

        Repeater {
          model: root.settingsList
          Item {
            width: parent.width
            implicitHeight: Math.max(keyText.implicitHeight, valueControl.implicitHeight)

            Text {
              id: keyText
              anchors.left: parent.left
              anchors.verticalCenter: parent.verticalCenter
              width: Style.space(150)
              elide: Text.ElideRight
              text: modelData.key
              color: modelData.value === modelData["default"] ? Color.foreground : Color.accent
              font.family: Style.font.family
              font.pixelSize: Style.font.caption

              MouseArea {
                id: keyMouse
                anchors.fill: parent
                hoverEnabled: true
              }

              PanelToolTip {
                visible: keyMouse.containsMouse
                text: modelData.about + " (default: " + modelData["default"] + ")"
              }
            }

            Item {
              id: valueControl
              anchors.left: keyText.right
              anchors.right: resetButton.left
              anchors.rightMargin: Style.space(4)
              anchors.verticalCenter: parent.verticalCenter
              implicitHeight: modelData.kind === "text" ? typed.implicitHeight : picked.implicitHeight

              Button {
                id: picked
                visible: modelData.kind !== "text"
                bordered: true
                text: modelData.value
                fontSize: Style.font.caption
                tooltipText: "Click: " + root.nextValue(modelData)
                onClicked: root.setValue(modelData.key, root.nextValue(modelData))
              }

              TextField {
                id: typed
                visible: modelData.kind === "text"
                width: parent.width
                text: modelData.value
                onAccepted: root.setValue(modelData.key, text)
              }
            }

            Button {
              id: resetButton
              anchors.right: parent.right
              anchors.verticalCenter: parent.verticalCenter
              visible: modelData.value !== modelData["default"]
              text: "default"
              fontSize: Style.font.caption
              onClicked: root.setValue(modelData.key, null)
            }
          }
        }

        Row {
          spacing: Style.space(6)
          Button { text: "Download everything"; fontSize: Style.font.caption; onClicked: root.download() }
        }

        Text {
          width: parent.width
          visible: root.settingsMessage !== ""
          wrapMode: Text.Wrap
          textFormat: Text.PlainText
          text: root.settingsMessage
          color: Color.accent
          font.family: Style.font.family
          font.pixelSize: Style.font.caption
        }
      }

      TextField {
        id: field
        width: parent.width
        placeholderText: "10 km mi  ·  12+12/3  ·  2026/12/04 - 12d  ·  now unix"
        onTextChanged: {
          root.query = text
          root.submitted = false
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
          color: root.missing || (root.reply && root.reply.show !== "result") ? Color.muted : Color.foreground
          font.family: Style.font.family
          font.pixelSize: root.reply && root.reply.show === "result" ? Style.font.subtitle : Style.font.body
          text: root.missing
            ? "`" + root.command + "` not found: install aequalis (see docs/omarchy-widget.md)"
            : !root.reply || root.reply.show === "none" ? ""
            : root.reply.show === "result" ? "= " + root.reply.result
            : root.reply.message || ""
        }

        Button {
          id: copyButton
          anchors.right: parent.right
          anchors.verticalCenter: parent.verticalCenter
          visible: !root.missing && root.reply !== null && root.reply.show === "result"
          iconText: "\uf0c5"  // Nerd Font: copy
          text: "Copy"
          fontSize: Style.font.caption
          tooltipText: visible ? "Copy " + root.reply.copy + " to the clipboard" : ""
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
        text: root.reply && root.reply.show === "result" ? (root.reply.details || []).join("\n") : ""
      }

      // An ambiguous date: one button per reading; the chosen pattern is written in. A misspelt
      // city: one button per city it may be. A country with several time zones: one per zone, its
      // most relevant city first, then its offset now. A shared or misspelt currency (`dollar`,
      // `dolar`): one per currency it may be, its name and code.
      Flow {
        width: parent.width
        spacing: Style.space(6)
        visible: root.reply !== null && root.reply.show === "question"
        Repeater {
          model: root.reply && root.reply.show === "question" ? root.reply.choices : []
          Button {
            bordered: true
            // Its label, and more about it as a tooltip (a place's population and map link).
            text: modelData.label
            tooltipText: modelData.detail
            onClicked: root.use(modelData.request)
          }
        }
      }

      // Notices: a new city archive, out-of-date data (the `notify` refresh policies).
      Repeater {
        model: root.notices
        Row {
          spacing: Style.space(8)
          Text {
            anchors.verticalCenter: parent.verticalCenter
            text: modelData
            color: Color.accent
            font.family: Style.font.family
            font.pixelSize: Style.font.caption
          }
          Button {
            text: "Download"
            fontSize: Style.font.caption
            onClicked: root.download()
          }
        }
      }

      Text {
        visible: syncProc.running
        text: "Downloading… (" + root.settingsMessage.replace(/^Downloading /, "").replace(/…$/, "") + ")"
        color: Color.muted
        font.family: Style.font.family
        font.pixelSize: Style.font.caption
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
        text: "Enter or Copy copies · Esc closes · click Aequalis for About"
        color: Color.muted
        font.family: Style.font.family
        font.pixelSize: Style.font.caption
      }
    }
  }
}
