import AppKit

// `Aequalis --selftest "10 km mi"` prints the answer and exits: the release build's check, which
// needs no screen. 0 when the core answered.
let arguments = CommandLine.arguments
if let at = arguments.firstIndex(of: "--selftest") {
    Core.start()
    let reply = Core.view(arguments.dropFirst(at + 1).joined(separator: " "), submitted: true)
    print(reply.show == "result" ? reply.copy : "\(reply.show): \(reply.message)")
    exit(reply.show == "result" ? 0 : 1)
}

let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.setActivationPolicy(.accessory)
app.run()
