import AppKit
import Carbon.HIToolbox
import SwiftUI

/// Tally in the menu bar: an icon that opens the panel (`PanelView`), also with ⌃⌥T from anywhere.
/// No Dock icon, no main window (`LSUIElement`).
@main
@MainActor
enum Main {
    static func main() {
        let app = NSApplication.shared
        let delegate = AppDelegate()
        app.delegate = delegate
        app.setActivationPolicy(.accessory)
        app.run()
    }
}

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private var item: NSStatusItem?
    private let popover = NSPopover()
    private var model: PanelModel?
    private var hotKey: HotKey?

    func applicationDidFinishLaunching(_ notification: Notification) {
        let model = PanelModel()
        self.model = model
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        item.button?.image = Self.icon()
        item.button?.toolTip = "Tally (⌃⌥T)"
        item.button?.target = self
        item.button?.action = #selector(toggle(_:))
        self.item = item
        let hosting = NSHostingController(rootView: PanelView(model: model, close: { [weak self] in self?.close() }))
        hosting.sizingOptions = [.preferredContentSize]
        popover.contentViewController = hosting
        // Closes when clicking elsewhere, like the other menu bar items.
        popover.behavior = .transient
        hotKey = HotKey(keyCode: UInt32(kVK_ANSI_T), modifiers: UInt32(controlKey | optionKey)) { [weak self] in
            self?.toggle(nil)
        }
    }

    @objc func toggle(_ sender: Any?) {
        if popover.isShown {
            close()
        } else {
            show()
        }
    }

    private func show() {
        guard let button = item?.button else { return }
        NSApp.activate(ignoringOtherApps: true)
        popover.show(relativeTo: button.bounds, of: button, preferredEdge: .minY)
        popover.contentViewController?.view.window?.makeKey()
        model?.opened()
    }

    private func close() {
        popover.performClose(nil)
        model?.closed()
    }

    /// The menu bar icon: a tally gate (four strokes crossed by a fifth), drawn as a template so it
    /// follows the menu bar's light or dark look.
    static func icon() -> NSImage {
        let image = NSImage(size: NSSize(width: 18, height: 16), flipped: false) { _ in
            let path = NSBezierPath()
            path.lineWidth = 1.6
            path.lineCapStyle = .round
            for x in [4.0, 7.5, 11.0, 14.5] {
                path.move(to: NSPoint(x: x, y: 2.5))
                path.line(to: NSPoint(x: x, y: 13.5))
            }
            path.move(to: NSPoint(x: 1.5, y: 4))
            path.line(to: NSPoint(x: 16.5, y: 12))
            NSColor.black.setStroke()
            path.stroke()
            return true
        }
        image.isTemplate = true
        return image
    }
}

/// A global keyboard shortcut (Carbon's hot keys: no Accessibility permission needed).
final class HotKey {
    private var ref: EventHotKeyRef?
    private var handler: EventHandlerRef?
    private let action: @MainActor () -> Void

    init?(keyCode: UInt32, modifiers: UInt32, action: @escaping @MainActor () -> Void) {
        self.action = action
        var spec = EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed))
        let me = Unmanaged.passUnretained(self).toOpaque()
        let installed = InstallEventHandler(GetApplicationEventTarget(), { _, _, data in
            guard let data else { return noErr }
            let hotKey = Unmanaged<HotKey>.fromOpaque(data).takeUnretainedValue()
            Task { @MainActor in hotKey.action() }
            return noErr
        }, 1, &spec, me, &handler)
        guard installed == noErr else { return nil }
        // 'TALY'
        let id = EventHotKeyID(signature: OSType(0x5441_4C59), id: 1)
        guard RegisterEventHotKey(keyCode, modifiers, id, GetApplicationEventTarget(), 0, &ref) == noErr else { return nil }
    }

    deinit {
        if let ref { UnregisterEventHotKey(ref) }
        if let handler { RemoveEventHandler(handler) }
    }
}
