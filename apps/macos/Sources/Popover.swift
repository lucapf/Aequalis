import AppKit

/// One button carrying the request (a question's choice, a Recent one) or the word (a suggestion)
/// it writes.
private final class WriteButton: NSButton {
    var request = ""
}

/// The widget: a popover under the menu bar icon, as the Omarchy widget is under its bar icon. The
/// request is answered as it is typed (the core is in this process: a function call, no process
/// started); Enter or Copy copies the answer and closes it, Share sends `request = result`. The
/// title opens About, the terminal button the TUI, the gear the basic Settings.
final class Popover: NSObject, NSPopoverDelegate {
    private let popover = NSPopover()
    private let controller = WidgetController()
    private var lastClosed = Date.distantPast

    override init() {
        super.init()
        popover.behavior = .transient
        popover.animates = false
        popover.contentViewController = controller
        popover.delegate = self
        controller.close = { [weak self] in self?.popover.performClose(nil) }
        controller.resize = { [weak self] size in self?.popover.contentSize = size }
    }

    var isShown: Bool { popover.isShown }

    func toggle(under button: NSView) {
        if popover.isShown {
            popover.performClose(nil)
        } else if Date().timeIntervalSince(lastClosed) > 0.25 {
            // A click on the icon that closed it (the popover is transient) does not open it again.
            show(under: button)
        }
    }

    /// Opens it on the request box, or (`settings`) on the Settings.
    func show(under button: NSView, settings: Bool = false) {
        controller.prepare(settings: settings)
        NSApp.activate(ignoringOtherApps: true)
        popover.show(relativeTo: button.bounds, of: button, preferredEdge: .minY)
        controller.focus()
        Log.write("popover shown")
    }

    func popoverDidClose(_ notification: Notification) {
        lastClosed = Date()
        controller.closed()
        Log.write("popover hidden")
    }
}

/// What the popover shows: the title, the request, its answer, and About or the Settings.
final class WidgetController: NSViewController, NSTextFieldDelegate {
    private static let margin: CGFloat = 14
    /// Its width: 420 points, up to 640 for a table or a long result.
    private static let minWidth: CGFloat = 420
    private static let maxWidth: CGFloat = 640
    private static let shownChoices = 8
    private static let shownSuggestions = 5
    private static let recentKept = 6

    var close: () -> Void = {}
    var resize: (NSSize) -> Void = { _ in }

    private enum Section { case answer, about, settings }
    private var section = Section.answer

    private let stack = NSStackView()
    private var stackWidth: NSLayoutConstraint?
    private var width = WidgetController.minWidth

    private let titleButton = NSButton(title: "Aequalis", target: nil, action: nil)
    private let status = NSTextField(labelWithString: "")
    private let tui = NSButton()
    private let gear = NSButton()
    private let field = NSTextField()
    private let suggestions = NSStackView()
    private let answer = NSTextField(wrappingLabelWithString: "")
    private let copyButton = NSButton(title: "Copy", target: nil, action: nil)
    private let shareButton = NSButton(title: "Share", target: nil, action: nil)
    private let extras = NSTextField(wrappingLabelWithString: "")
    private let details = NSTextField(wrappingLabelWithString: "")
    private let choices = NSStackView()
    private let notices = NSStackView()
    private let progressLine = NSTextField(wrappingLabelWithString: "")
    private let progress = NSProgressIndicator()
    private let recentTitle = NSTextField(labelWithString: "Recent")
    private let recent = NSStackView()
    private let about = NSTextField(wrappingLabelWithString: "")
    private let aboutIcon = NSImageView()
    private let settingsView = SettingsView()
    private let hint = NSTextField(labelWithString: "Enter copies · Esc closes")

    private var reply = Reply()
    /// The request the reply answers.
    private var request = ""
    private var offered: (start: Int, items: [(write: String, label: String)]) = (start: 0, items: [])
    private var recents: [(request: String, result: String)] = []
    private var syncing = false
    /// The last sync's end, shown until the next one.
    private var syncEnd = ""
    private var timer: Timer?
    private var ticks = 0

    override func loadView() {
        let root = NSView()
        view = root

        titleButton.target = self
        titleButton.action = #selector(toggleAbout)
        titleButton.isBordered = false
        titleButton.font = .systemFont(ofSize: 13, weight: .semibold)
        status.font = .systemFont(ofSize: 11)
        status.textColor = .secondaryLabelColor
        symbol(tui, "terminal", "Open the TUI", #selector(openTUI))
        symbol(gear, "gearshape", "Settings", #selector(toggleSettings))
        let closeButton = NSButton()
        symbol(closeButton, "xmark", "Close", #selector(closeClicked))
        let spacer = NSView()
        spacer.setContentHuggingPriority(.defaultLow, for: .horizontal)
        let header = NSStackView(views: [titleButton, status, spacer, tui, gear, closeButton])
        header.spacing = 6

        field.isBordered = false
        field.drawsBackground = false
        field.focusRingType = .none
        field.font = .systemFont(ofSize: 22, weight: .light)
        field.placeholderString = "10 km mi"
        field.delegate = self
        field.cell?.isScrollable = true
        suggestions.spacing = 4

        answer.isSelectable = true
        for button in [copyButton, shareButton] {
            button.bezelStyle = .rounded
            button.controlSize = .small
            button.target = self
        }
        copyButton.action = #selector(copyClicked)
        shareButton.action = #selector(shareClicked)
        let actions = NSStackView(views: [copyButton, shareButton])
        actions.spacing = 6

        extras.font = .systemFont(ofSize: 13)
        details.font = .systemFont(ofSize: 11)
        details.textColor = .secondaryLabelColor
        for list in [choices, notices, recent] {
            list.orientation = .vertical
            list.alignment = .leading
            list.spacing = 2
        }
        progressLine.font = .systemFont(ofSize: 11)
        progress.style = .bar
        progress.minValue = 0
        progress.maxValue = 1
        recentTitle.font = .systemFont(ofSize: 11, weight: .semibold)
        recentTitle.textColor = .secondaryLabelColor
        about.font = .systemFont(ofSize: 12)
        about.isSelectable = true
        aboutIcon.image = NSApp.applicationIconImage
        aboutIcon.widthAnchor.constraint(equalToConstant: 48).isActive = true
        aboutIcon.heightAnchor.constraint(equalToConstant: 48).isActive = true
        settingsView.onResize = { [weak self] in self?.relayout() }
        settingsView.onSync = { [weak self] in self?.sync() }
        hint.font = .systemFont(ofSize: 10)
        hint.textColor = .tertiaryLabelColor

        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 8
        let views: [NSView] = [header, field, suggestions, answer, actions, extras, details, choices, notices,
                               progressLine, progress, recentTitle, recent, aboutIcon, about, settingsView, hint]
        views.forEach { stack.addArrangedSubview($0) }
        stack.translatesAutoresizingMaskIntoConstraints = false
        root.addSubview(stack)
        let stackWidth = stack.widthAnchor.constraint(equalToConstant: width - 2 * WidgetController.margin)
        self.stackWidth = stackWidth
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: root.leadingAnchor, constant: WidgetController.margin),
            stack.topAnchor.constraint(equalTo: root.topAnchor, constant: WidgetController.margin),
            stackWidth,
            header.widthAnchor.constraint(equalTo: stack.widthAnchor),
            field.widthAnchor.constraint(equalTo: stack.widthAnchor),
            progress.widthAnchor.constraint(equalTo: stack.widthAnchor),
        ])
    }

    private func symbol(_ button: NSButton, _ name: String, _ tip: String, _ action: Selector) {
        button.image = NSImage(systemSymbolName: name, accessibilityDescription: tip)
        button.isBordered = false
        button.toolTip = tip
        button.target = self
        button.action = action
    }

    // ------------------------------------------------------------ opening and closing

    /// Before it shows: an empty request, the online state, the TUI's button; the Settings when asked.
    func prepare(settings: Bool) {
        _ = view
        field.stringValue = ""
        request = ""
        reply = Reply()
        offered = (start: 0, items: [])
        section = settings ? .settings : .answer
        if settings { settingsView.reload() }
        refreshStatus()
        render()
        startTimer()
    }

    func focus() {
        view.window?.makeFirstResponder(section == .settings ? nil : field)
    }

    /// Closed: the timer stops, unless a sync runs (its end is still to be read).
    func closed() {
        if !syncing { stopTimer() }
    }

    /// One timer while it shows or a sync runs: the sync's progress every second, the TUI's button
    /// every 2 seconds, the online state every 30.
    private func startTimer() {
        guard timer == nil else { return }
        ticks = 0
        let timer = Timer(timeInterval: 1, repeats: true) { [weak self] _ in self?.tick() }
        RunLoop.main.add(timer, forMode: .common)
        self.timer = timer
    }

    private func stopTimer() {
        timer?.invalidate()
        timer = nil
    }

    private func tick() {
        ticks += 1
        let shown = view.window?.isVisible == true
        if syncing {
            readSync()
        } else if !shown {
            stopTimer()
            return
        }
        if shown && ticks % 2 == 0 { tui.isEnabled = !Terminal.isOpen }
        if shown && ticks % 30 == 0 { status.stringValue = Core.online() }
    }

    private func refreshStatus() {
        status.stringValue = Core.online()
        tui.isEnabled = !Terminal.isOpen
    }

    // ------------------------------------------------------------ what it shows

    private func render() {
        let answering = section == .answer
        answer.stringValue = ""
        answer.textColor = .labelColor
        extras.stringValue = ""
        details.stringValue = ""
        choices.arrangedSubviews.forEach { $0.removeFromSuperview() }
        notices.arrangedSubviews.forEach { $0.removeFromSuperview() }

        let table = reply.isTable
        answer.font = table ? .monospacedSystemFont(ofSize: 12, weight: .regular) : .systemFont(ofSize: 20, weight: .semibold)
        switch reply.show {
        case "result":
            if let calendar = reply.calendar, let font = answer.font {
                answer.attributedStringValue = CalendarText.render(reply.result, calendar, font: font)
            } else {
                answer.stringValue = table ? reply.result : "= " + reply.result
            }
            extras.stringValue = reply.extras.joined(separator: "\n")
            details.stringValue = ([reply.grouping] + reply.details.map { Optional($0) } + [reply.rates]).compactMap { $0 }.joined(separator: "\n")
        case "hint":
            answer.stringValue = reply.message
            answer.font = .systemFont(ofSize: 13)
            answer.textColor = .secondaryLabelColor
            extras.stringValue = reply.extras.joined(separator: "\n")
        case "error":
            answer.stringValue = reply.message
            answer.font = .systemFont(ofSize: 13)
            answer.textColor = .systemRed
        case "question":
            answer.stringValue = reply.message
            answer.font = .systemFont(ofSize: 13)
            for choice in reply.choices.prefix(WidgetController.shownChoices) {
                let button = WriteButton(title: choice.label, target: self, action: #selector(use(_:)))
                button.bezelStyle = .rounded
                button.toolTip = choice.detail.isEmpty ? nil : choice.detail
                button.request = choice.request
                choices.addArrangedSubview(button)
            }
        default:
            break
        }
        for notice in reply.notices {
            let line = NSTextField(labelWithString: notice)
            line.font = .systemFont(ofSize: 11)
            line.textColor = .controlAccentColor
            let row = NSStackView(views: [line, plainButton("Sync", #selector(syncClicked))])
            row.spacing = 8
            notices.addArrangedSubview(row)
        }
        renderSuggestions()
        renderRecent()
        renderAbout()

        let isResult = reply.show == "result"
        answer.isHidden = !answering || answer.stringValue.isEmpty
        copyButton.superview?.isHidden = !answering || !isResult
        extras.isHidden = !answering || extras.stringValue.isEmpty
        details.isHidden = !answering || details.stringValue.isEmpty
        choices.isHidden = !answering || choices.arrangedSubviews.isEmpty
        notices.isHidden = !answering || notices.arrangedSubviews.isEmpty
        recentTitle.isHidden = !answering || recents.isEmpty
        recent.isHidden = recentTitle.isHidden
        suggestions.isHidden = !answering || suggestions.arrangedSubviews.isEmpty
        aboutIcon.isHidden = section != .about
        about.isHidden = section != .about
        settingsView.isHidden = section != .settings
        hint.isHidden = !answering
        renderSync()
        relayout()
    }

    private func renderSuggestions() {
        suggestions.arrangedSubviews.forEach { $0.removeFromSuperview() }
        for (i, item) in offered.items.prefix(WidgetController.shownSuggestions).enumerated() {
            let button = WriteButton(title: item.label, target: self, action: #selector(suggestionClicked(_:)))
            button.bezelStyle = .inline
            button.controlSize = .small
            button.request = item.write
            if i == 0 { button.toolTip = "Tab" }
            suggestions.addArrangedSubview(button)
        }
    }

    private func renderRecent() {
        recent.arrangedSubviews.forEach { $0.removeFromSuperview() }
        for entry in recents {
            let button = WriteButton(title: "\(entry.request)  = \(entry.result)", target: self, action: #selector(use(_:)))
            button.isBordered = false
            button.font = .systemFont(ofSize: 12)
            button.contentTintColor = .secondaryLabelColor
            button.request = entry.request
            recent.addArrangedSubview(button)
        }
    }

    /// About: the version and the facts of the core (the last online sync, the data's sources).
    private func renderAbout() {
        guard section == .about else { return }
        let facts = Core.about()
        let app = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? facts.version
        let wanted = ["Last online sync", "Exchange rates", "Cities"]
        let lines = ["Aequalis \(app)", "Type it. Get it."] + facts.facts.filter { wanted.contains($0.0) }.map { "\($0.0): \($0.1)" }
        about.stringValue = lines.joined(separator: "\n")
    }

    /// The sync: what it is doing and how far, then how it ended.
    private func renderSync() {
        progress.isHidden = !syncing
        progressLine.isHidden = !syncing && syncEnd.isEmpty
        if !syncing { progressLine.stringValue = syncEnd }
    }

    /// Fits the popover to what it shows: as wide as a table or a long result needs (420 to 640
    /// points), as tall as its content.
    private func relayout() {
        var wanted = WidgetController.minWidth
        if section == .answer, reply.show == "result", let font = answer.font {
            let widest = answer.stringValue.split(separator: "\n").map { (String($0) as NSString).size(withAttributes: [.font: font]).width }.max() ?? 0
            wanted = min(WidgetController.maxWidth, max(wanted, ceil(widest) + 2 * WidgetController.margin + 8))
        }
        width = wanted
        let inner = width - 2 * WidgetController.margin
        stackWidth?.constant = inner
        for label in [answer, extras, details, progressLine, about] { label.preferredMaxLayoutWidth = inner }
        settingsView.width = inner
        view.layoutSubtreeIfNeeded()
        let size = NSSize(width: width, height: stack.fittingSize.height + 2 * WidgetController.margin)
        view.setFrameSize(size)
        resize(size)
    }

    // ------------------------------------------------------------ typing

    func controlTextDidChange(_ notification: Notification) {
        guard notification.object as? NSTextField === field else { return }
        answerTyped()
    }

    /// The answer of what is in the box, and the words to offer for the one being typed.
    private func answerTyped() {
        request = field.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        reply = request.isEmpty ? Reply() : Core.view(request, submitted: false)
        if request.isEmpty {
            offered = (start: 0, items: [])
        } else {
            offered = Core.suggest(textBeforeCursor())
        }
        if section != .answer { section = .answer }
        render()
    }

    private func textBeforeCursor() -> String {
        let text = field.stringValue as NSString
        let cursor = min(field.currentEditor()?.selectedRange.location ?? text.length, text.length)
        return text.substring(to: cursor)
    }

    func control(_ control: NSControl, textView: NSTextView, doCommandBy selector: Selector) -> Bool {
        switch selector {
        case #selector(NSResponder.insertNewline(_:)):
            submit()
        case #selector(NSResponder.cancelOperation(_:)):
            close()
        case #selector(NSResponder.insertTab(_:)):
            guard let first = offered.items.first else { return false }
            write(first.write)
        default:
            return false
        }
        return true
    }

    /// Enter: a result is copied and the popover closes; a question takes its default; otherwise
    /// what the request lacks is said.
    private func submit() {
        guard !request.isEmpty else { return }
        reply = Core.view(request, submitted: true)
        switch reply.show {
        case "result":
            copyAndClose()
        case "question":
            if let chosen = reply.defaultRequest { setRequest(chosen) } else { render() }
        default:
            render()
        }
    }

    private func copyAndClose() {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(reply.copy, forType: .string)
        Log.write("copied: \(reply.copy)")
        recents.removeAll { $0.request == request }
        recents.insert((request, reply.copy.replacingOccurrences(of: "\n", with: " · ")), at: 0)
        if recents.count > WidgetController.recentKept { recents.removeLast() }
        close()
    }

    /// A suggestion takes the place of the word being typed, a space after it, as on Android.
    private func write(_ word: String) {
        let before = textBeforeCursor()
        let after = String((field.stringValue as NSString).substring(from: (before as NSString).length).drop { $0 == " " })
        // The core counts the word's start in Unicode scalars.
        let start = before.unicodeScalars.index(before.unicodeScalars.startIndex, offsetBy: min(offered.start, before.unicodeScalars.count))
        var head = String(before.unicodeScalars[..<start])
        if !head.isEmpty && !head.hasSuffix(" ") { head += " " }
        head += word + " "
        field.stringValue = head + after
        field.currentEditor()?.selectedRange = NSRange(location: (head as NSString).length, length: 0)
        answerTyped()
    }

    private func setRequest(_ text: String) {
        view.window?.makeFirstResponder(field)
        field.stringValue = text
        field.currentEditor()?.selectedRange = NSRange(location: (text as NSString).length, length: 0)
        answerTyped()
    }

    // ------------------------------------------------------------ buttons

    @objc private func use(_ sender: WriteButton) { setRequest(sender.request) }

    @objc private func suggestionClicked(_ sender: WriteButton) {
        view.window?.makeFirstResponder(field)
        write(sender.request)
    }

    @objc private func copyClicked() {
        guard reply.show == "result" else { return }
        copyAndClose()
    }

    /// Share: `request = result` (`10 km mi = 6.2137 mi`), through the Mac's share menu.
    @objc private func shareClicked() {
        guard reply.show == "result" else { return }
        let text = "\(request) = \(reply.result)"
        NSSharingServicePicker(items: [text]).show(relativeTo: shareButton.bounds, of: shareButton, preferredEdge: .minY)
        Log.write("shared: \(text)")
    }

    @objc private func openTUI() {
        let error = Terminal.openTUI()
        tui.isEnabled = false
        if error.isEmpty { close() } else { syncEnd = "TUI: \(error)"; render() }
    }

    @objc private func toggleAbout() {
        section = section == .about ? .answer : .about
        render()
    }

    @objc private func toggleSettings() {
        section = section == .settings ? .answer : .settings
        if section == .settings { settingsView.reload() }
        render()
        if section == .answer { view.window?.makeFirstResponder(field) }
    }

    @objc private func closeClicked() { close() }

    @objc private func syncClicked() { sync() }

    // ------------------------------------------------------------ the sync

    /// Downloads everything off the main thread; its progress is read every second.
    func sync() {
        guard !syncing else { return }
        syncing = true
        syncEnd = ""
        startTimer()
        readSync()
        DispatchQueue.global(qos: .utility).async {
            Core.sync(automatic: false)
            DispatchQueue.main.async { [weak self] in
                guard let self = self else { return }
                self.syncing = false
                self.readSync()
                self.refreshStatus()
                if self.section == .settings { self.settingsView.reload() }
                if !self.request.isEmpty { self.reply = Core.view(self.request, submitted: false) }
                self.render()
            }
        }
        render()
    }

    private func readSync() {
        let state = Core.syncStatus()
        let line = state["line"] as? String ?? ""
        if syncing {
            progressLine.textColor = .labelColor
            progressLine.stringValue = line.isEmpty ? (state["label"] as? String ?? "Sync") : line
            if let fraction = state["fraction"] as? Double {
                progress.isIndeterminate = false
                progress.doubleValue = fraction
            } else {
                progress.isIndeterminate = true
                progress.startAnimation(nil)
            }
        } else {
            progress.stopAnimation(nil)
            let message = state["message"] as? String ?? ""
            syncEnd = message.isEmpty ? line : message
            progressLine.textColor = state["state"] as? String == "failed" ? .systemRed : .controlAccentColor
        }
        if view.window?.isVisible == true { renderSync(); relayout() }
    }

    private func plainButton(_ title: String, _ action: Selector) -> NSButton {
        let button = NSButton(title: title, target: self, action: action)
        button.bezelStyle = .inline
        button.controlSize = .small
        return button
    }
}
