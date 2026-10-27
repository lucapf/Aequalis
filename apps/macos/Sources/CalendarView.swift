import AppKit

/// The calendar (`cal`) drawn as on Android, from the reply's `calendar`: a month's grid (the
/// weekdays on a grey band that goes on down the weekend's columns, today in a circle of the Mac's
/// accent colour, a holiday in its first country's colour with a dot a country under it), or the
/// year as small months, three a row; under it the countries and (a month) its holidays. A click on
/// a holiday shows its holidays; ‹ ›, ← → (once clicked) or a swipe on the trackpad go to the month
/// (the year) before or after. Copy keeps the core's text.
final class CalendarView: NSView {
    /// Asked with the request of the month (the year) before or after.
    var onStep: (String) -> Void = { _ in }
    /// Called when its height changes (a holiday's details shown or hidden).
    var onResize: () -> Void = {}

    /// The width it is drawn at.
    var width: CGFloat = 392 {
        didSet { if width != oldValue { invalidateIntrinsicContentSize() } }
    }

    private var calendar = Model()
    /// The day clicked (month, day): its holidays under the grid.
    private var selected: (month: Int, day: Int)?
    /// Where each holiday's day is drawn, to find the one clicked.
    private var spots: [(rect: NSRect, month: Int, day: Int)] = []
    private let previous = NSButton()
    private let next = NSButton()
    /// The trackpad's horizontal scroll so far in this gesture.
    private var swiped: CGFloat = 0

    override var isFlipped: Bool { true }
    override var acceptsFirstResponder: Bool { true }

    init() {
        super.init(frame: .zero)
        for (button, name, tip, action) in [(previous, "chevron.left", "Before (←)", #selector(stepBack)), (next, "chevron.right", "After (→)", #selector(stepOn))] {
            button.image = NSImage(systemSymbolName: name, accessibilityDescription: tip)
            button.isBordered = false
            button.toolTip = tip
            button.target = self
            button.action = action
            addSubview(button)
        }
    }

    required init?(coder: NSCoder) { fatalError("not used") }

    func show(_ json: [String: Any]) {
        calendar = Model(json)
        selected = nil
        invalidateIntrinsicContentSize()
        needsDisplay = true
    }

    override var intrinsicContentSize: NSSize { NSSize(width: width, height: walk(drawing: false)) }

    override func draw(_ dirtyRect: NSRect) {
        _ = walk(drawing: true)
    }

    // ------------------------------------------------------------ colours

    private var dark: Bool { effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua }

    /// The countries' colours, in order (red, blue, green, purple, teal, as the terminal's and
    /// Android's), lighter on a dark screen.
    private func colour(_ country: Int) -> NSColor {
        let light: [UInt32] = [0xD32F2F, 0x1565C0, 0x2E7D32, 0x7B1FA2, 0x00838F]
        let night: [UInt32] = [0xEF9A9A, 0x90CAF9, 0xA5D6A7, 0xCE93D8, 0x80DEEA]
        let rgb = (dark ? night : light)[country % light.count]
        return NSColor(srgbRed: CGFloat(rgb >> 16 & 0xFF) / 255, green: CGFloat(rgb >> 8 & 0xFF) / 255, blue: CGFloat(rgb & 0xFF) / 255, alpha: 1)
    }

    private var band: NSColor { NSColor.labelColor.withAlphaComponent(dark ? 0.10 : 0.06) }

    // ------------------------------------------------------------ drawing

    /// Lays the calendar out (and with `drawing`, draws it); its height.
    private func walk(drawing: Bool) -> CGFloat {
        if drawing { spots = [] }
        guard !calendar.months.isEmpty else { return 0 }
        var y: CGFloat = 0

        // The title, ‹ › at its right.
        let title = calendar.title as NSString
        let titleFont = NSFont.systemFont(ofSize: calendar.year ? 16 : 14, weight: .bold)
        if drawing {
            title.draw(at: NSPoint(x: 0, y: y), withAttributes: [.font: titleFont, .foregroundColor: NSColor.labelColor])
            previous.frame = NSRect(x: width - 44, y: y, width: 20, height: 20)
            next.frame = NSRect(x: width - 20, y: y, width: 20, height: 20)
            previous.isEnabled = !calendar.previous.isEmpty
            next.isEnabled = !calendar.next.isEmpty
        }
        y += 26

        if calendar.year {
            let gap: CGFloat = 12
            let small = (width - 2 * gap) / 3
            for row in stride(from: 0, to: calendar.months.count, by: 3) {
                var height: CGFloat = 0
                for (j, month) in calendar.months[row..<min(row + 3, calendar.months.count)].enumerated() {
                    height = max(height, grid(month, x: CGFloat(j) * (small + gap), y: y, width: small, compact: true, drawing: drawing))
                }
                y += height + 10
            }
        } else {
            y += grid(calendar.months[0], x: 0, y: y, width: width, compact: false, drawing: drawing) + 10
        }

        // The day clicked: its holidays, each with its country's dot and how long it lasts.
        if let chosen = selected {
            let on = calendar.holidays.filter { $0.month == chosen.month && $0.day == chosen.day }
            if let first = on.first {
                y += line(first.date, x: 0, y: y, font: .systemFont(ofSize: 12, weight: .semibold), colour: .labelColor, drawing: drawing)
                for h in on {
                    let long = h.length > 1 ? "\(h.length) days · \(h.days)" : "1 day"
                    if drawing { dot(h.country, at: NSPoint(x: 4, y: y + 8), size: 8) }
                    y += line("\(h.name) (\(calendar.countries[safe: h.country]?.name ?? "")) · \(long)", x: 16, y: y, font: .systemFont(ofSize: 12), colour: .labelColor, drawing: drawing)
                }
                y += 6
            }
        }

        // The countries, a dot each in its colour, as many a line as fit.
        if !calendar.countries.isEmpty {
            var x: CGFloat = 0
            let font = NSFont.systemFont(ofSize: 12)
            for (i, country) in calendar.countries.enumerated() {
                let text = "\(country.name) (\(country.code))" as NSString
                let w = 14 + text.size(withAttributes: [.font: font]).width
                if x > 0 && x + w > width {
                    x = 0
                    y += 18
                }
                if drawing {
                    dot(i, at: NSPoint(x: x + 5, y: y + 8), size: 10)
                    text.draw(at: NSPoint(x: x + 14, y: y), withAttributes: [.font: font, .foregroundColor: NSColor.labelColor])
                }
                x += w + 14
            }
            y += 20
        }

        // A month's holidays, one a holiday, its days together.
        if !calendar.year {
            let font = NSFont.systemFont(ofSize: 12)
            var seen = Set<String>()
            for h in calendar.holidays where seen.insert("\(h.country)|\(h.first)|\(h.name)").inserted {
                if drawing {
                    dot(h.country, at: NSPoint(x: 4, y: y + 8), size: 8)
                    (h.days.isEmpty ? "\(h.day)" : h.days).draw(at: NSPoint(x: 16, y: y), withAttributes: [.font: font, .foregroundColor: NSColor.secondaryLabelColor])
                }
                y += line(h.length > 1 ? "\(h.name) (\(h.length) days)" : h.name, x: 90, y: y, font: font, colour: .labelColor, drawing: drawing)
            }
        }
        return ceil(y)
    }

    /// A month: its title (in the year), the weekdays on their band, a row a week with its number.
    /// Its height.
    private func grid(_ m: Model.Month, x: CGFloat, y top: CGFloat, width: CGFloat, compact: Bool, drawing: Bool) -> CGFloat {
        let unit = width / 7.8
        let wk = unit * 0.8
        let rowHeight: CGFloat = compact ? 16 : 34
        let headerHeight: CGFloat = compact ? 15 : 22
        let circle: CGFloat = compact ? 15 : 26
        let dayFont = NSFont.monospacedDigitSystemFont(ofSize: compact ? 10 : 13, weight: .regular)
        let boldFont = NSFont.monospacedDigitSystemFont(ofSize: compact ? 10 : 13, weight: .bold)
        let small = NSFont.systemFont(ofSize: compact ? 8 : 10, weight: .semibold)
        var y = top

        if compact {
            if drawing { (m.title as NSString).draw(at: NSPoint(x: x, y: y), withAttributes: [.font: NSFont.systemFont(ofSize: 11, weight: .semibold), .foregroundColor: NSColor.labelColor]) }
            y += 17
        }
        let gridHeight = headerHeight + rowHeight * CGFloat(m.weeks.count)
        if drawing {
            // The band: the weekdays, and the weekend's columns on down.
            band.setFill()
            NSBezierPath(roundedRect: NSRect(x: x, y: y, width: width, height: headerHeight), xRadius: 5, yRadius: 5).fill()
            for k in calendar.weekend {
                NSBezierPath(rect: NSRect(x: x + wk + CGFloat(k) * unit, y: y + headerHeight, width: unit, height: gridHeight - headerHeight)).fill()
            }
            centred("Wk", in: NSRect(x: x, y: y, width: wk, height: headerHeight), font: small, colour: .tertiaryLabelColor)
            for (k, name) in calendar.weekdays.enumerated() {
                let shown = compact ? String(name.prefix(1)) : name
                centred(shown, in: NSRect(x: x + wk + CGFloat(k) * unit, y: y, width: unit, height: headerHeight), font: small,
                        colour: calendar.weekend.contains(k) ? .tertiaryLabelColor : .secondaryLabelColor)
            }
        }
        y += headerHeight

        for week in m.weeks {
            if drawing {
                centred("\(week.number)", in: NSRect(x: x, y: y, width: wk, height: compact ? rowHeight : circle), font: small, colour: .tertiaryLabelColor)
            }
            for (k, day) in week.days.enumerated() where day > 0 && drawing {
                let cell = NSRect(x: x + wk + CGFloat(k) * unit, y: y, width: unit, height: rowHeight)
                let ring = NSRect(x: cell.midX - circle / 2, y: y + (compact ? (rowHeight - circle) / 2 : 2), width: circle, height: circle)
                let countries = calendar.countriesOn(m.month, day)
                let today = calendar.isToday(m, day)
                var textColour: NSColor = calendar.weekend.contains(k) ? .secondaryLabelColor : .labelColor
                if let first = countries.first { textColour = colour(first) }
                if today {
                    NSColor.controlAccentColor.setFill()
                    NSBezierPath(ovalIn: ring).fill()
                    textColour = .white
                }
                if let chosen = selected, chosen.month == m.month, chosen.day == day {
                    colour(countries.first ?? 0).setStroke()
                    let path = NSBezierPath(ovalIn: ring.insetBy(dx: -1, dy: -1))
                    path.lineWidth = 2
                    path.stroke()
                }
                var attributes: [NSAttributedString.Key: Any] = [.font: today || !countries.isEmpty ? boldFont : dayFont, .foregroundColor: textColour]
                // Small months have no room for the dots: several countries' holiday underlined.
                if compact && countries.count > 1 { attributes[.underlineStyle] = NSUnderlineStyle.single.rawValue }
                centred("\(day)", in: ring, attributes)
                if !compact && !countries.isEmpty {
                    let dots = CGFloat(countries.count) * 7 - 2
                    for (i, country) in countries.enumerated() {
                        dot(country, at: NSPoint(x: cell.midX - dots / 2 + CGFloat(i) * 7 + 2.5, y: ring.maxY + 3), size: 5)
                    }
                }
                if !countries.isEmpty { spots.append((cell, m.month, day)) }
            }
            y += rowHeight
        }
        return y - top
    }

    private func centred(_ text: String, in rect: NSRect, font: NSFont, colour: NSColor) {
        centred(text, in: rect, [.font: font, .foregroundColor: colour])
    }

    private func centred(_ text: String, in rect: NSRect, _ attributes: [NSAttributedString.Key: Any]) {
        let string = text as NSString
        let size = string.size(withAttributes: attributes)
        string.draw(at: NSPoint(x: rect.midX - size.width / 2, y: rect.midY - size.height / 2), withAttributes: attributes)
    }

    private func dot(_ country: Int, at centre: NSPoint, size: CGFloat) {
        colour(country).setFill()
        NSBezierPath(ovalIn: NSRect(x: centre.x - size / 2, y: centre.y - size / 2, width: size, height: size)).fill()
    }

    /// A line of text, wrapped to the width left; its height.
    private func line(_ text: String, x: CGFloat, y: CGFloat, font: NSFont, colour: NSColor, drawing: Bool) -> CGFloat {
        let attributes: [NSAttributedString.Key: Any] = [.font: font, .foregroundColor: colour]
        let bounds = (text as NSString).boundingRect(with: NSSize(width: width - x, height: 1000), options: [.usesLineFragmentOrigin], attributes: attributes)
        if drawing { (text as NSString).draw(with: NSRect(x: x, y: y, width: width - x, height: ceil(bounds.height)), options: [.usesLineFragmentOrigin], attributes: attributes) }
        return ceil(bounds.height) + 3
    }

    // ------------------------------------------------------------ clicks, keys, swipes

    /// A click on a holiday shows its holidays; on it again, or anywhere else, hides them.
    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
        let point = convert(event.locationInWindow, from: nil)
        let hit = spots.first { $0.rect.contains(point) }
        let was = selected
        if let hit = hit, !(was?.month == hit.month && was?.day == hit.day) {
            selected = (hit.month, hit.day)
        } else {
            selected = nil
        }
        if was?.month != selected?.month || was?.day != selected?.day {
            invalidateIntrinsicContentSize()
            needsDisplay = true
            onResize()
        }
    }

    override func keyDown(with event: NSEvent) {
        switch event.keyCode {
        case 123: stepBack()  // ←
        case 124: stepOn()  // →
        default: super.keyDown(with: event)
        }
    }

    /// Two fingers across the trackpad: to the left the month after, to the right the one before.
    override func scrollWheel(with event: NSEvent) {
        guard event.hasPreciseScrollingDeltas, abs(event.scrollingDeltaX) > abs(event.scrollingDeltaY) || swiped != 0 else {
            super.scrollWheel(with: event)
            return
        }
        if event.phase == .began { swiped = 0 }
        swiped += event.scrollingDeltaX
        if event.phase == .ended || event.phase == .cancelled {
            if swiped < -60 { stepOn() } else if swiped > 60 { stepBack() }
            swiped = 0
        }
    }

    @objc private func stepBack() {
        if !calendar.previous.isEmpty { onStep(calendar.previous) }
    }

    @objc private func stepOn() {
        if !calendar.next.isEmpty { onStep(calendar.next) }
    }
}

/// The reply's `calendar`.
private struct Model {
    struct Month {
        var title = ""
        var year = 0
        var month = 0
        var weeks: [(number: Int, days: [Int])] = []
    }

    struct Holiday {
        var date = ""
        var day = 0
        var month = 0
        var country = 0
        var name = ""
        var first = ""
        var length = 1
        var days = ""
    }

    var title = ""
    var year = false
    var today = ""
    var weekdays: [String] = []
    var weekend: Set<Int> = []
    var countries: [(code: String, name: String)] = []
    var holidays: [Holiday] = []
    var months: [Month] = []
    var previous = ""
    var next = ""
    /// The countries with a holiday each day (`month * 32 + day`), in their order.
    private var byDay: [Int: [Int]] = [:]

    init() {}

    init(_ json: [String: Any]) {
        title = json["title"] as? String ?? ""
        year = json["year"] as? Bool ?? false
        today = json["today"] as? String ?? ""
        weekdays = json["weekdays"] as? [String] ?? []
        weekend = Set(json["weekend"] as? [Int] ?? [])
        previous = json["previous"] as? String ?? ""
        next = json["next"] as? String ?? ""
        countries = (json["countries"] as? [[String: Any]] ?? []).map { (code: $0["code"] as? String ?? "", name: $0["name"] as? String ?? "") }
        holidays = (json["holidays"] as? [[String: Any]] ?? []).map { h in
            Holiday(date: h["date"] as? String ?? "", day: h["day"] as? Int ?? 0, month: h["month"] as? Int ?? 0, country: h["country"] as? Int ?? 0,
                    name: h["name"] as? String ?? "", first: h["first"] as? String ?? "", length: h["length"] as? Int ?? 1, days: h["days"] as? String ?? "")
        }
        months = (json["months"] as? [[String: Any]] ?? []).map { m in
            Month(title: m["title"] as? String ?? "", year: m["year"] as? Int ?? 0, month: m["month"] as? Int ?? 0,
                  weeks: (m["weeks"] as? [[String: Any]] ?? []).map { (number: $0["week"] as? Int ?? 0, days: $0["days"] as? [Int] ?? []) })
        }
        for h in holidays where !(byDay[h.month * 32 + h.day] ?? []).contains(h.country) {
            byDay[h.month * 32 + h.day, default: []].append(h.country)
        }
        for key in byDay.keys { byDay[key]?.sort() }
    }

    func countriesOn(_ month: Int, _ day: Int) -> [Int] { byDay[month * 32 + day] ?? [] }

    func isToday(_ m: Month, _ day: Int) -> Bool {
        today == String(format: "%04d-%02d-%02d", m.year, m.month, day)
    }
}

private extension Array {
    subscript(safe index: Int) -> Element? { indices.contains(index) ? self[index] : nil }
}
