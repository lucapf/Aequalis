import XCTest
@testable import Tally

/// The core through the C interface, in the simulator, and the app's rules.
final class TallyTests: XCTestCase {
    func testRequests() {
        XCTAssertEqual(Engine.eval("10 km mi").result, "6.2137 mi")
        XCTAssertEqual(Engine.eval("12+4/2").result, "14")
        let error = Engine.eval("10 foo bar")
        XCTAssertFalse(error.ok)
        XCTAssertNotNil(error.error)
        // An ambiguous date: its readings.
        let date = Engine.eval("12/04/2026 unix")
        XCTAssertEqual(date.ambiguous?.kind, "date")
        XCTAssertGreaterThan(date.ambiguous?.choices.count ?? 0, 1)
        // The time zones work (the system's zone files, or the copy in the app).
        XCTAssertTrue(Engine.eval("10:00 Tokyo to Rome").ok, Engine.eval("10:00 Tokyo to Rome").error ?? "")
    }

    func testLists() {
        XCTAssertTrue(Engine.measures().contains { $0.name == "Length" && $0.units.contains { $0.symbol == "km" } })
        XCTAssertTrue(Engine.currencies().contains { $0.code == "EUR" })
        XCTAssertEqual(Engine.trivia().first?.name, "Earth")
        XCTAssertTrue(Engine.settings().contains { $0.key == "offline" && $0.kind == "bool" })
    }

    func testSettingsAreSaved() {
        XCTAssertNil(Engine.set("precision", "2"))
        XCTAssertEqual(Engine.settings().first { $0.key == "precision" }?.value, "2")
        XCTAssertNotNil(Engine.set("precision", "99"), "out of range: the error")
        XCTAssertNil(Engine.set("precision", nil))
    }

    func testTheTypingRule() {
        let error = Reply(ok: false, error: "`Rie` is not a city")
        XCTAssertTrue(isQuiet(error, query: "Rie", submitted: false))
        XCTAssertFalse(isQuiet(error, query: "Rie ", submitted: false), "after a space")
        XCTAssertFalse(isQuiet(error, query: "Rie", submitted: true), "after Return")
        XCTAssertFalse(isQuiet(Reply(ok: true, result: "16"), query: "12+4", submitted: false), "results at once")
    }

    func testValuesAndUnits() {
        XCTAssertEqual(valueOf("2026/11/22 YYYY/MM/DD"), "2026/11/22")
        XCTAssertEqual(valueOf("6.2137 mi"), "6.2137 mi")
        XCTAssertEqual(adding("km", to: ""), "1 km ")
        XCTAssertEqual(adding("mi", to: "10 km "), "10 km mi ")
        let recent = remember(Entry(input: "a", result: "1"), in: [Entry(input: "b", result: "2"), Entry(input: "a", result: "0")])
        XCTAssertEqual(recent.map(\.input), ["a", "b"])
    }

    func testNoticesForTheApp() throws {
        let json = #"{"ok":true,"result":"1","notices":["the city archive is not downloaded yet: tally --sync"]}"#
        let reply = try JSONDecoder().decode(Reply.self, from: Data(json.utf8))
        XCTAssertEqual(reply.appNotices, ["the city archive is not downloaded yet (Settings › Download everything)"])
    }
}
