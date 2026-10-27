import AppKit

/// The calendar (`cal`) as the interactive mode draws it: the core's text, with today marked, the
/// weekend dimmed and each country's holidays in its colour (underlined when several countries
/// share the day), from the reply's `calendar`. The text, and so Copy, stays the core's.
enum CalendarText {
    /// The countries' colours, in the core's order (`calendar::COLOURS`: red, blue, green, magenta, cyan).
    private static let colours: [NSColor] = [.systemRed, .systemBlue, .systemGreen, .systemPurple, .systemTeal]
    /// A month's width (`calendar::WIDTH`: `Wk` and 7 days of 3 characters), and the gap between
    /// the months of a year's row.
    private static let width = 24
    private static let gap = 2

    static func render(_ text: String, _ calendar: [String: Any], font: NSFont) -> NSAttributedString {
        let out = NSMutableAttributedString(string: text, attributes: [.font: font, .foregroundColor: NSColor.labelColor])
        let lines = text.components(separatedBy: "\n")
        // Where each line starts, in UTF-16 units (the attributed string's).
        var starts: [Int] = []
        var at = 0
        for line in lines {
            starts.append(at)
            at += (line as NSString).length + 1
        }
        let bold = NSFontManager.shared.convert(font, toHaveTrait: .boldFontMask)

        func style(line: Int, column: Int, length: Int, _ attributes: [NSAttributedString.Key: Any]) {
            guard line < lines.count, column + length <= (lines[line] as NSString).length else { return }
            out.addAttributes(attributes, range: NSRange(location: starts[line] + column, length: length))
        }

        let today = calendar["today"] as? String ?? ""
        let weekend = Set(calendar["weekend"] as? [Int] ?? [])
        // Each day's holidays: the countries (their index) with one that day.
        var holidays: [String: [Int]] = [:]
        for holiday in calendar["holidays"] as? [[String: Any]] ?? [] {
            guard let date = holiday["date"] as? String, let country = holiday["country"] as? Int else { continue }
            if !(holidays[date] ?? []).contains(country) { holidays[date, default: []].append(country) }
        }

        // The months: alone, from the first line; a year's three a row, after its title, each row
        // after a blank line and as high as its longest month.
        let months = calendar["months"] as? [[String: Any]] ?? []
        let year = calendar["year"] as? Bool ?? false
        var line = year ? 1 : 0
        var last = 0
        for row in stride(from: 0, to: months.count, by: year ? 3 : 1) {
            if year { line += 1 }
            let block = Array(months[row..<min(row + (year ? 3 : 1), months.count)])
            var height = 0
            for (j, month) in block.enumerated() {
                let weeks = month["weeks"] as? [[String: Any]] ?? []
                height = max(height, 2 + weeks.count)
                let y = month["year"] as? Int ?? 0
                let m = month["month"] as? Int ?? 0
                for (w, week) in weeks.enumerated() {
                    for (k, day) in (week["days"] as? [Int] ?? []).enumerated() where day > 0 {
                        let date = String(format: "%04d-%02d-%02d", y, m, day)
                        var attributes: [NSAttributedString.Key: Any] = [:]
                        if let countries = holidays[date], let first = countries.first {
                            attributes[.foregroundColor] = colours[first % colours.count]
                            attributes[.font] = bold
                            if countries.count > 1 { attributes[.underlineStyle] = NSUnderlineStyle.single.rawValue }
                        } else if weekend.contains(k) {
                            attributes[.foregroundColor] = NSColor.secondaryLabelColor
                        }
                        if date == today {
                            attributes[.backgroundColor] = NSColor.controlAccentColor
                            attributes[.foregroundColor] = NSColor.white
                            attributes[.font] = bold
                        }
                        if !attributes.isEmpty {
                            style(line: line + 2 + w, column: j * (width + gap) + 3 + 3 * k, length: 2, attributes)
                        }
                    }
                }
            }
            line += height
            last = line
        }

        // Under the months: the countries, then (a month) their holidays; each code in its colour.
        let countries = (calendar["countries"] as? [[String: Any]] ?? []).compactMap { $0["code"] as? String }
        for index in last..<lines.count {
            let text = lines[index] as NSString
            for (i, code) in countries.enumerated() {
                var search = NSRange(location: 0, length: text.length)
                while true {
                    let found = text.range(of: code, options: [], range: search)
                    guard found.location != NSNotFound else { break }
                    let before = found.location == 0 || text.substring(with: NSRange(location: found.location - 1, length: 1)) == " "
                    let afterAt = found.location + found.length
                    let after = afterAt == text.length || text.substring(with: NSRange(location: afterAt, length: 1)) == " "
                    if before && after {
                        style(line: index, column: found.location, length: found.length, [.foregroundColor: colours[i % colours.count], .font: bold])
                    }
                    search = NSRange(location: afterAt, length: text.length - afterAt)
                }
            }
        }
        return out
    }
}
