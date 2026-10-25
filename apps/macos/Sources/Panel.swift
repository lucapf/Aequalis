import AppKit

/// One button of a question: the request it puts in the box.
private final class ChoiceButton: NSButton {
    var request = ""
}

/// The input box: a floating panel with the request, its answer under it as you type, and one
/// button per reading when the request can be read in more than one way. Enter copies a result (or
/// takes a question's default), Esc or a click anywhere else closes it.
final class Panel: NSPanel, NSTextFieldDelegate {
    private static let width: CGFloat = 560
    private static let margin: CGFloat = 14
    /// Choices shown at most: a place's name can have dozens.
    private static let shownChoices = 8

    private let field = NSTextField()
    private let answer = NSTextField(labelWithString: "")
    private let note = NSTextField(wrappingLabelWithString: "")
    private let choices = NSStackView()
    private let stack = NSStackView()
    private var lastHidden = Date.distantPast

    init() {
        super.init(contentRect: NSRect(x: 0, y: 0, width: Panel.width, height: 64), styleMask: [.borderless], backing: .buffered, defer: false)
        level = .floating
        isOpaque = false
        backgroundColor = .clear
        hasShadow = true
        isReleasedWhenClosed = false
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]

        let background = NSVisualEffectView()
        background.material = .hudWindow
        background.state = .active
        background.blendingMode = .behindWindow
        background.wantsLayer = true
        background.layer?.cornerRadius = 12
        background.layer?.masksToBounds = true
        contentView = background

        let inner = Panel.width - 2 * Panel.margin
        field.isBordered = false
        field.drawsBackground = false
        field.focusRingType = .none
        field.font = .systemFont(ofSize: 24, weight: .light)
        field.placeholderString = "10 km mi"
        field.delegate = self
        answer.font = .systemFont(ofSize: 20, weight: .semibold)
        answer.isSelectable = true
        answer.lineBreakMode = .byTruncatingTail
        note.font = .systemFont(ofSize: 12)
        note.textColor = .secondaryLabelColor
        note.preferredMaxLayoutWidth = inner
        choices.orientation = .vertical
        choices.alignment = .leading
        choices.spacing = 4

        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 8
        for view in [field, answer, note, choices] as [NSView] { stack.addArrangedSubview(view) }
        stack.translatesAutoresizingMaskIntoConstraints = false
        background.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: background.leadingAnchor, constant: Panel.margin),
            stack.topAnchor.constraint(equalTo: background.topAnchor, constant: Panel.margin),
            stack.widthAnchor.constraint(equalToConstant: inner),
            field.widthAnchor.constraint(equalToConstant: inner),
        ])

        NotificationCenter.default.addObserver(forName: NSWindow.didResignKeyNotification, object: self, queue: .main) { [weak self] _ in
            self?.dismiss()
        }
        render(Reply())
    }

    override var canBecomeKey: Bool { true }

    // ------------------------------------------------------------ showing

    func toggle() {
        if isVisible {
            dismiss()
        } else if Date().timeIntervalSince(lastHidden) > 0.25 {
            // A click on the menu bar icon that took the focus has just hidden it: not shown again.
            present()
        }
    }

    func present() {
        field.stringValue = ""
        render(Reply())
        let screen = NSScreen.screens.first { NSMouseInRect(NSEvent.mouseLocation, $0.frame, false) } ?? NSScreen.main
        if let area = screen?.visibleFrame {
            setFrameOrigin(NSPoint(x: area.midX - frame.width / 2, y: area.maxY - area.height * 0.22 - frame.height))
        }
        NSApp.activate(ignoringOtherApps: true)
        makeKeyAndOrderFront(nil)
        makeFirstResponder(field)
        Log.write("panel shown")
    }

    func dismiss() {
        guard isVisible else { return }
        orderOut(nil)
        lastHidden = Date()
        Log.write("panel hidden")
    }

    // ------------------------------------------------------------ what it shows

    private func render(_ reply: Reply) {
        answer.stringValue = ""
        note.stringValue = ""
        note.textColor = .secondaryLabelColor
        choices.arrangedSubviews.forEach { $0.removeFromSuperview() }
        switch reply.show {
        case "result":
            answer.stringValue = reply.result
            note.stringValue = ([reply.grouping] + reply.details.map { Optional($0) } + [reply.rates]).compactMap { $0 }.joined(separator: "\n")
        case "hint":
            note.stringValue = reply.message
        case "error":
            note.stringValue = reply.message
            note.textColor = .systemRed
        case "question":
            note.stringValue = reply.message
            for choice in reply.choices.prefix(Panel.shownChoices) {
                let button = ChoiceButton(title: choice.label, target: self, action: #selector(choose(_:)))
                button.bezelStyle = .rounded
                button.request = choice.request
                choices.addArrangedSubview(button)
            }
        default:
            break
        }
        answer.isHidden = answer.stringValue.isEmpty
        note.isHidden = note.stringValue.isEmpty
        choices.isHidden = choices.arrangedSubviews.isEmpty
        relayout()
    }

    /// Fits the panel to what it shows, its top staying where it is.
    private func relayout() {
        contentView?.layoutSubtreeIfNeeded()
        let height = stack.fittingSize.height + 2 * Panel.margin
        let top = frame.maxY
        setFrame(NSRect(x: frame.minX, y: top - height, width: Panel.width, height: height), display: true)
    }

    private func setRequest(_ request: String) {
        field.stringValue = request
        field.currentEditor()?.selectedRange = NSRange(location: (request as NSString).length, length: 0)
        render(Core.view(request, submitted: false))
    }

    @objc private func choose(_ sender: ChoiceButton) {
        makeFirstResponder(field)
        setRequest(sender.request)
    }

    // ------------------------------------------------------------ typing

    func controlTextDidChange(_ notification: Notification) {
        let text = field.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        render(text.isEmpty ? Reply() : Core.view(text, submitted: false))
    }

    func control(_ control: NSControl, textView: NSTextView, doCommandBy selector: Selector) -> Bool {
        if selector == #selector(NSResponder.insertNewline(_:)) {
            submit()
            return true
        }
        if selector == #selector(NSResponder.cancelOperation(_:)) {
            dismiss()
            return true
        }
        return false
    }

    /// Enter: a result is copied; a question takes its default; otherwise what the request lacks is said.
    private func submit() {
        let text = field.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return }
        let reply = Core.view(text, submitted: true)
        switch reply.show {
        case "result":
            Panel.copy(reply.copy)
            Log.write("copied: \(reply.copy)")
            dismiss()
        case "question":
            if let request = reply.defaultRequest { setRequest(request) } else { render(reply) }
        default:
            render(reply)
        }
    }

    static func copy(_ text: String) {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(text, forType: .string)
    }
}
