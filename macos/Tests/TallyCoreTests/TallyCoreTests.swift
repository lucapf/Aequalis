import XCTest
@testable import TallyCore

/// Replies as the real CLI prints them (`TALLY_MODE=offline tally --json …`).
final class TallyCoreTests: XCTestCase {
    func reply(_ json: String) throws -> Reply {
        try XCTUnwrap(Reply.decode(Data(json.utf8)), json)
    }

    func testResult() throws {
        let r = try reply(#"{"ok":true,"result":"6.2137 mi","notes":[],"warnings":["w"],"peak_kb":4176}"#)
        XCTAssertTrue(r.ok)
        XCTAssertEqual(r.headline, "= 6.2137 mi")
        XCTAssertEqual(r.peakKb, 4176)
        XCTAssertEqual(r.remarks, ["w"])
    }

    func testError() throws {
        let r = try reply(#"{"ok":false,"exit":1,"error":"division by zero","peak_kb":3960}"#)
        XCTAssertEqual(r.headline, "division by zero")
        XCTAssertEqual(r.exit, 1)
        XCTAssertEqual(r.remarks, [])
    }

    func testAmbiguousDate() throws {
        let r = try reply(#"{"ok":false,"exit":2,"error":"…","ambiguous":{"kind":"date","text":"12/04/2026","choices":[{"pattern":"MM/DD/YYYY","date":"2026-12-04","request":"12/04/2026 MM/DD/YYYY unix"},{"pattern":"DD/MM/YYYY","date":"2026-04-12","request":"12/04/2026 DD/MM/YYYY unix"}]},"peak_kb":3988}"#)
        XCTAssertEqual(r.headline, "Which date is 12/04/2026?")
        XCTAssertEqual(r.ambiguous?.choices.map(\.request), ["12/04/2026 MM/DD/YYYY unix", "12/04/2026 DD/MM/YYYY unix"])
        XCTAssertEqual(r.ambiguous?.choices.first?.label, "MM/DD/YYYY   2026-12-04")
    }

    func testMisspeltPlaces() throws {
        let r = try reply(#"{"ok":false,"exit":2,"error":"…","ambiguous":{"kind":"city","text":"jappan, romme","note":"`jappan`, `romme` are not known places","choices":[{"pattern":"Japan (country), Rome (city)","date":"Asia/Tokyo, Europe/Rome","request":"10:00am Japan Rome"}]},"peak_kb":4052}"#)
        XCTAssertEqual(r.headline, "`jappan`, `romme` are not known places: did you mean…")
        let old = try reply(#"{"ok":false,"error":"…","ambiguous":{"kind":"city","text":"Tokio","choices":[{"request":"Tokyo"}]}}"#)
        XCTAssertEqual(old.headline, "`Tokio` is not a city: did you mean…")
    }

    func testCountry() throws {
        let r = try reply(#"{"ok":false,"exit":2,"error":"…","ambiguous":{"kind":"country","text":"usa","choices":[{"pattern":"New York","date":"GMT-4 · America/New_York","request":"now New York"}]},"peak_kb":4064}"#)
        XCTAssertEqual(r.headline, "`usa` has several time zones: which one?")
    }

    func testWords() {
        XCTAssertEqual(words("  10 km\tmi \n"), ["10", "km", "mi"])
        XCTAssertEqual(words("   "), [])
    }

    func testValueOf() {
        XCTAssertEqual(valueOf("6.2137 mi"), "6.2137 mi")
        XCTAssertEqual(valueOf("2026/11/22 YYYY/MM/DD"), "2026/11/22")
        XCTAssertEqual(valueOf("18/09/2026 DD/MM/YYYY-24h-tz"), "18/09/2026")
        XCTAssertEqual(valueOf("1791741920 unix"), "1791741920")
        XCTAssertEqual(valueOf("2026-10-12T16:19:02+02:00 iso"), "2026-10-12T16:19:02+02:00")
        XCTAssertEqual(valueOf("2026-10-11T10:00+02:00 Rome = 2026-10-11T04:00-04:00 New York"), "2026-10-11T04:00-04:00")
        XCTAssertEqual(valueOf("14"), "14")
    }

    func testSize() {
        XCTAssertEqual(size(kb: 900), "900 KiB")
        XCTAssertEqual(size(kb: 2970), "2.9 MiB")
    }

    func testRemember() {
        let a = Entry(input: "1+1", result: "2")
        let b = Entry(input: "10 km mi", result: "6.2137 mi")
        XCTAssertEqual(remember(a, in: [b, a]), [a, b])
        let many = (0..<10).map { Entry(input: "\($0)", result: "\($0)") }
        XCTAssertEqual(remember(a, in: many).count, 6)
    }

    func testLocate() {
        let home = "/Users/me"
        let present: Set<String> = ["/opt/homebrew/bin/tally", "/Users/me/.local/bin/tally", "/custom/tally", "/App/tally"]
        let exists = { present.contains($0) }
        XCTAssertEqual(locateTally(setting: "/custom/tally", bundled: "/App/tally", home: home, isExecutable: exists), "/custom/tally")
        XCTAssertEqual(locateTally(setting: " ", bundled: "/App/tally", home: home, isExecutable: exists), "/App/tally")
        XCTAssertEqual(locateTally(setting: nil, bundled: nil, home: home, isExecutable: exists), "/Users/me/.local/bin/tally")
        XCTAssertNil(locateTally(setting: "/missing", bundled: nil, home: home, isExecutable: { _ in false }))
    }

    /// The real CLI, when the tests are given one (`TALLY_BIN`, set by CI).
    func testRealCli() async throws {
        guard let path = ProcessInfo.processInfo.environment["TALLY_BIN"] else {
            throw XCTSkip("TALLY_BIN not set")
        }
        let tally = Tally(path: path, offline: true)
        let result = await tally.json(["10", "km", "mi"])
        XCTAssertEqual(result?.headline, "= 6.2137 mi")
        let ambiguous = await tally.json(["12/04/2026", "unix"])
        XCTAssertEqual(ambiguous?.ambiguous?.choices.count, 2)
        let online = await tally.json(["--online"])
        XCTAssertEqual(online?.result, "offline (forced)")
        let none = await Tally(path: "/nonexistent/tally", offline: true).json(["1"])
        XCTAssertNil(none)
    }
}
