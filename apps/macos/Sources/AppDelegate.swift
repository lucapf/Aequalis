import AppKit
import Carbon
import ServiceManagement

/// The menu bar icon and its menu, the global hotkey (⌥⌘A), the popover under the icon and the
/// two Services.
final class AppDelegate: NSObject, NSApplicationDelegate {
    private let popover = Popover()
    private let hotkey = Hotkey()
    private var item: NSStatusItem?
    private let menu = NSMenu()
    private let loginItem = NSMenuItem(title: "Open at login", action: #selector(toggleLogin), keyEquivalent: "")

    func applicationDidFinishLaunching(_ notification: Notification) {
        Log.write("Aequalis \(version) starting, macOS \(ProcessInfo.processInfo.operatingSystemVersionString)")
        Core.start()

        let status = hotkey.register(keyCode: kVK_ANSI_A, modifiers: optionKey | cmdKey) { [weak self] in self?.togglePopover() }
        Log.write(status == noErr ? "hotkey ⌥⌘A registered" : "hotkey ⌥⌘A not registered (status \(status)): another app has it")

        buildMenu()
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        if let button = item.button {
            if let icon = NSImage(systemSymbolName: "equal.circle", accessibilityDescription: "Aequalis") {
                button.image = icon
            } else {
                button.title = "="
            }
            button.target = self
            button.action = #selector(statusClicked)
            button.sendAction(on: [.leftMouseUp, .rightMouseUp])
        }
        self.item = item

        NSApp.servicesProvider = self
        NSUpdateDynamicServices()

        // The online data, as the settings say (rates, the city archive), off the main thread.
        DispatchQueue.global(qos: .utility).async { Core.sync(automatic: true) }
    }

    private var version: String {
        Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "?"
    }

    // ------------------------------------------------------------ the menu bar

    private func buildMenu() {
        menu.addItem(withTitle: "Open", action: #selector(openPanel), keyEquivalent: "").target = self
        menu.addItem(.separator())
        menu.addItem(withTitle: "Download data", action: #selector(downloadData), keyEquivalent: "").target = self
        menu.addItem(withTitle: "Settings", action: #selector(openSettings), keyEquivalent: "").target = self
        loginItem.target = self
        if #available(macOS 13.0, *) {
            loginItem.state = SMAppService.mainApp.status == .enabled ? .on : .off
        } else {
            loginItem.isEnabled = false
        }
        menu.addItem(loginItem)
        menu.addItem(.separator())
        menu.addItem(withTitle: "About", action: #selector(about), keyEquivalent: "").target = self
        menu.addItem(withTitle: "Log", action: #selector(openLog), keyEquivalent: "").target = self
        menu.addItem(withTitle: "Quit", action: #selector(quit), keyEquivalent: "q").target = self
    }

    /// A click opens the popover, a right-click (or Control-click) the menu.
    @objc private func statusClicked() {
        let event = NSApp.currentEvent
        if event?.type == .rightMouseUp || event?.modifierFlags.contains(.control) == true {
            item?.menu = menu
            item?.button?.performClick(nil)
            item?.menu = nil
        } else {
            togglePopover()
        }
    }

    private func togglePopover() {
        guard let button = item?.button else { return }
        popover.toggle(under: button)
    }

    @objc private func openPanel() {
        guard let button = item?.button, !popover.isShown else { return }
        popover.show(under: button)
    }

    @objc private func downloadData() {
        DispatchQueue.global(qos: .utility).async {
            let error = Core.sync(automatic: false)
            DispatchQueue.main.async {
                self.notice(error.isEmpty ? "Data downloaded" : "Download failed", error)
            }
        }
    }

    /// The popover's Settings (the basic ones; the rest in the TUI).
    @objc private func openSettings() {
        guard let button = item?.button else { return }
        popover.show(under: button, settings: true)
    }

    @objc private func toggleLogin() {
        guard #available(macOS 13.0, *) else { return }
        do {
            if SMAppService.mainApp.status == .enabled {
                try SMAppService.mainApp.unregister()
            } else {
                try SMAppService.mainApp.register()
            }
        } catch {
            Log.write("open at login failed: \(error)")
            notice("Open at login failed", "\(error.localizedDescription)")
        }
        loginItem.state = SMAppService.mainApp.status == .enabled ? .on : .off
    }

    @objc private func about() {
        NSApp.activate(ignoringOtherApps: true)
        NSApp.orderFrontStandardAboutPanel(nil)
    }

    @objc private func openLog() { run("/usr/bin/open", ["-t", Log.url.path]) }

    @objc private func quit() { NSApp.terminate(nil) }

    private func run(_ tool: String, _ arguments: [String]) {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: tool)
        process.arguments = arguments
        do { try process.run() } catch { Log.write("\(tool) failed: \(error)") }
    }

    private func notice(_ title: String, _ text: String) {
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = text
        NSApp.activate(ignoringOtherApps: true)
        alert.runModal()
    }

    // ------------------------------------------------------------ Services

    /// The answer to the selected text; a question takes its default. `nil` and a message when
    /// there is none.
    private func answer(_ pasteboard: NSPasteboard) -> (text: String?, error: String) {
        guard let request = pasteboard.string(forType: .string)?.trimmingCharacters(in: .whitespacesAndNewlines), !request.isEmpty else {
            return (nil, "Nothing selected")
        }
        var reply = Core.view(request, submitted: true)
        if reply.show == "question", let chosen = reply.defaultRequest {
            reply = Core.view(chosen, submitted: true)
        }
        guard reply.show == "result" else { return (nil, reply.message.isEmpty ? "No answer" : reply.message) }
        return (reply.copy, "")
    }

    /// Service: the selection is replaced by its answer.
    @objc func replaceWithAnswer(_ pasteboard: NSPasteboard, userData: String, error: AutoreleasingUnsafeMutablePointer<NSString>) {
        let found = answer(pasteboard)
        Log.write("service replace: \(found.text ?? found.error)")
        guard let text = found.text else {
            error.pointee = found.error as NSString
            return
        }
        pasteboard.clearContents()
        pasteboard.setString(text, forType: .string)
    }

    /// Service: the answer to the selection goes to the clipboard.
    @objc func copyAnswer(_ pasteboard: NSPasteboard, userData: String, error: AutoreleasingUnsafeMutablePointer<NSString>) {
        let found = answer(pasteboard)
        Log.write("service copy: \(found.text ?? found.error)")
        guard let text = found.text else {
            error.pointee = found.error as NSString
            return
        }
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
    }
}
