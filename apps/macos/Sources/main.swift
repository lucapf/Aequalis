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

// `Aequalis --selftest-speed`: each request of the app's kinds answered as typed, many times, and
// the slowest time said; 1 when one took over 5 ms (every keystroke is a call into the core).
if arguments.contains("--selftest-speed") {
    Core.start()
    let requests = ["10 km mi", "12+12/3", "100 usd eur", "2026/10/05 + 5wd", "cal 2027", "42 eu shoes", "time in tokyo"]
    let limit = 5.0
    var slowest = 0.0
    for request in requests {
        _ = Core.view(request, submitted: false)  // the first call loads what the request needs
        var worst = 0.0
        for _ in 0..<50 {
            let start = DispatchTime.now().uptimeNanoseconds
            _ = Core.view(request, submitted: false)
            _ = Core.suggest(request)
            worst = max(worst, Double(DispatchTime.now().uptimeNanoseconds - start) / 1_000_000)
        }
        print(request.padding(toLength: 20, withPad: " ", startingAt: 0) + String(format: "%.2f ms", worst))
        slowest = max(slowest, worst)
    }
    print(String(format: "slowest %.2f ms (limit %.0f ms)", slowest, limit))
    exit(slowest <= limit ? 0 : 1)
}

let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.setActivationPolicy(.accessory)
app.run()
