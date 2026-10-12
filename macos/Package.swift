// swift-tools-version:5.9
// Tally's menu bar app for macOS: a menu bar icon opening a panel that evaluates requests with the
// tally CLI (`tally --json …`), like the Omarchy bar widget (omarchy-widget/Panel.qml).
// TallyCore holds the logic (testable without a UI), TallyBar the AppKit / SwiftUI app.
import PackageDescription

let package = Package(
    name: "TallyBar",
    platforms: [.macOS(.v13)],
    targets: [
        .target(name: "TallyCore"),
        .executableTarget(name: "TallyBar", dependencies: ["TallyCore"]),
        .testTarget(name: "TallyCoreTests", dependencies: ["TallyCore"]),
    ]
)
