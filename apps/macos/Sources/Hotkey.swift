import Carbon

/// A global hotkey through Carbon's `RegisterEventHotKey`: it needs no Accessibility permission.
final class Hotkey {
    private static var action: (() -> Void)?
    private var ref: EventHotKeyRef?

    /// Registers the key; returns Carbon's status (`noErr`: done; otherwise another app has the key).
    func register(keyCode: Int, modifiers: Int, action: @escaping () -> Void) -> OSStatus {
        Hotkey.action = action
        var spec = EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed))
        let installed = InstallEventHandler(GetApplicationEventTarget(), { _, _, _ in
            Hotkey.action?()
            return noErr
        }, 1, &spec, nil, nil)
        if installed != noErr { return installed }
        let id = EventHotKeyID(signature: OSType(0x4145_5131), id: 1)
        return RegisterEventHotKey(UInt32(keyCode), UInt32(modifiers), id, GetApplicationEventTarget(), 0, &ref)
    }
}
