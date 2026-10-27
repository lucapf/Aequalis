import AppKit

/// One button of the settings: the setting (or the choice) it is for.
private final class KeyButton: NSButton {
    var key = ""
    var value = ""
}

/// The popover's Settings (the gear): the basic settings (the core's, not advanced), a choice or a
/// currency picked from a filterable list, a text written and saved with Enter, each with Default;
/// Reset (clicked twice) and Sync. The rest is in the TUI.
final class SettingsView: NSStackView, NSTextFieldDelegate, NSSearchFieldDelegate {
    /// Choices shown at most while filtering: the currencies are over 150.
    private static let shownChoices = 8

    /// Called when what it shows changes size, and when Sync is clicked.
    var onResize: () -> Void = {}
    var onSync: () -> Void = {}

    private var settings: [[String: Any]] = []
    /// The setting whose choices are open (empty: none).
    private var picking = ""
    private let filter = NSSearchField()
    private lazy var filterWidth = filter.widthAnchor.constraint(equalToConstant: width)
    private let options = NSStackView()
    private let message = NSTextField(wrappingLabelWithString: "")
    private let reset = NSButton(title: "Reset", target: nil, action: nil)
    private var resetAsked = Date.distantPast

    init() {
        super.init(frame: .zero)
        orientation = .vertical
        alignment = .leading
        spacing = 6
        filter.placeholderString = "Filter"
        filter.delegate = self
        filter.sendsSearchStringImmediately = true
        filterWidth.isActive = true
        options.orientation = .vertical
        options.alignment = .leading
        options.spacing = 2
        message.font = .systemFont(ofSize: 11)
        message.textColor = .secondaryLabelColor
        reset.target = self
        reset.action = #selector(resetClicked)
        reset.bezelStyle = .rounded
    }

    required init?(coder: NSCoder) { fatalError("not used") }

    /// Reads the settings again and shows them.
    func reload() {
        settings = Core.settings().filter { ($0["advanced"] as? Bool) == false }
        picking = ""
        message.stringValue = ""
        rebuild()
    }

    /// The width its wrapping text gets.
    var width: CGFloat = 392 {
        didSet {
            message.preferredMaxLayoutWidth = width
            filterWidth.constant = width
        }
    }

    // ------------------------------------------------------------ the rows

    private func rebuild() {
        arrangedSubviews.forEach { $0.removeFromSuperview() }
        for setting in settings {
            addArrangedSubview(row(setting))
            if string(setting, "key") == picking {
                addArrangedSubview(filter)
                addArrangedSubview(options)
                refreshOptions()
            }
        }
        let buttons = NSStackView(views: [button("Sync", #selector(syncClicked)), reset])
        buttons.spacing = 8
        reset.title = "Reset"
        addArrangedSubview(buttons)
        message.isHidden = message.stringValue.isEmpty
        addArrangedSubview(message)
        onResize()
        if !picking.isEmpty { window?.makeFirstResponder(filter) }
    }

    /// A setting: its name, its value (a switch, a button opening the choices, or a text field), and
    /// Default when it is not the default. What it does as the tooltip.
    private func row(_ setting: [String: Any]) -> NSView {
        let key = string(setting, "key")
        let value = string(setting, "value")
        let name = NSTextField(labelWithString: key)
        name.font = .systemFont(ofSize: 12)
        name.toolTip = [string(setting, "about"), string(setting, "detail")].filter { !$0.isEmpty }.joined(separator: "\n")
        name.widthAnchor.constraint(equalToConstant: 130).isActive = true
        var views: [NSView] = [name]

        switch string(setting, "kind") {
        case "bool":
            let check = KeyButton(checkboxWithTitle: "", target: self, action: #selector(toggled(_:)))
            check.key = key
            check.state = value == "true" ? .on : .off
            views.append(check)
        case "choice":
            views.append(pickButton(key, shown(setting)))
        default:
            if setting["currencies"] != nil {
                views.append(pickButton(key, shown(setting)))
            } else {
                let field = NSTextField(string: value)
                field.identifier = NSUserInterfaceItemIdentifier(key)
                field.delegate = self
                field.font = .systemFont(ofSize: 12)
                field.widthAnchor.constraint(equalToConstant: 120).isActive = true
                views.append(field)
            }
        }
        if value != string(setting, "default") {
            let back = KeyButton(title: "Default", target: self, action: #selector(toDefault(_:)))
            back.key = key
            back.bezelStyle = .inline
            back.controlSize = .small
            views.append(back)
        }
        let row = NSStackView(views: views)
        row.spacing = 8
        return row
    }

    private func pickButton(_ key: String, _ title: String) -> NSView {
        let pick = KeyButton(title: title, target: self, action: #selector(openChoices(_:)))
        pick.key = key
        pick.bezelStyle = .rounded
        pick.controlSize = .small
        return pick
    }

    /// The value as shown: `auto (EUR)` for auto.
    private func shown(_ setting: [String: Any]) -> String {
        let value = string(setting, "value")
        let meant = string(setting, "auto")
        return value == "auto" && !meant.isEmpty ? "auto (\(meant))" : value
    }

    // ------------------------------------------------------------ the choices

    /// The choices of the setting being picked, filtered: `(value written, label)`. A currency:
    /// auto first, then each currency's code and name.
    private func choices() -> [(String, String)] {
        guard let setting = settings.first(where: { string($0, "key") == picking }) else { return [] }
        var all: [(String, String)]
        if let currencies = setting["currencies"] as? [[String: Any]] {
            let meant = string(setting, "auto")
            all = [("auto", meant.isEmpty ? "auto" : "auto (\(meant))")]
            all += currencies.map { (string($0, "code"), "\(string($0, "code")) - \(string($0, "name"))") }
        } else {
            all = (setting["options"] as? [String] ?? []).map { ($0, $0 == "auto" && !string(setting, "auto").isEmpty ? "auto (\(string(setting, "auto")))" : $0) }
        }
        let typed = filter.stringValue.trimmingCharacters(in: .whitespaces)
        if !typed.isEmpty { all = all.filter { $0.1.localizedCaseInsensitiveContains(typed) } }
        return Array(all.prefix(SettingsView.shownChoices))
    }

    private func refreshOptions() {
        options.arrangedSubviews.forEach { $0.removeFromSuperview() }
        for (value, label) in choices() {
            let choice = KeyButton(title: label, target: self, action: #selector(chosen(_:)))
            choice.key = picking
            choice.value = value
            choice.bezelStyle = .inline
            options.addArrangedSubview(choice)
        }
        onResize()
    }

    @objc private func openChoices(_ sender: KeyButton) {
        picking = picking == sender.key ? "" : sender.key
        filter.stringValue = ""
        rebuild()
    }

    @objc private func chosen(_ sender: KeyButton) { change(sender.key, sender.value) }

    func controlTextDidChange(_ notification: Notification) {
        if notification.object as? NSSearchField === filter { refreshOptions() }
    }

    /// Enter in the filter takes the first choice; Esc closes the choices. Enter in a text setting
    /// saves it.
    func control(_ control: NSControl, textView: NSTextView, doCommandBy selector: Selector) -> Bool {
        if control === filter {
            if selector == #selector(NSResponder.insertNewline(_:)), let first = choices().first {
                change(picking, first.0)
                return true
            }
            if selector == #selector(NSResponder.cancelOperation(_:)) {
                picking = ""
                rebuild()
                return true
            }
            return false
        }
        if selector == #selector(NSResponder.insertNewline(_:)), let key = control.identifier?.rawValue {
            change(key, control.stringValue.trimmingCharacters(in: .whitespaces))
            return true
        }
        return false
    }

    // ------------------------------------------------------------ changes

    @objc private func toggled(_ sender: KeyButton) { change(sender.key, sender.state == .on ? "true" : "false") }

    @objc private func toDefault(_ sender: KeyButton) { change(sender.key, nil) }

    private func change(_ key: String, _ value: String?) {
        let error = Core.set(key, value)
        settings = Core.settings().filter { ($0["advanced"] as? Bool) == false }
        picking = ""
        message.textColor = error.isEmpty ? .secondaryLabelColor : .systemRed
        message.stringValue = error.isEmpty ? "Saved" : error
        rebuild()
    }

    /// Reset: the first click asks, a second within 3 seconds puts every default back.
    @objc private func resetClicked() {
        if Date().timeIntervalSince(resetAsked) > 3 {
            resetAsked = Date()
            reset.title = "Sure?"
            return
        }
        resetAsked = .distantPast
        let error = Core.reset()
        settings = Core.settings().filter { ($0["advanced"] as? Bool) == false }
        picking = ""
        message.textColor = error.isEmpty ? .secondaryLabelColor : .systemRed
        message.stringValue = error.isEmpty ? "Defaults" : error
        rebuild()
    }

    @objc private func syncClicked() { onSync() }

    private func button(_ title: String, _ action: Selector) -> NSButton {
        let button = NSButton(title: title, target: self, action: action)
        button.bezelStyle = .rounded
        return button
    }

    private func string(_ object: [String: Any], _ key: String) -> String {
        if let text = object[key] as? String { return text }
        if let number = object[key] as? NSNumber { return number.stringValue }
        return ""
    }
}
