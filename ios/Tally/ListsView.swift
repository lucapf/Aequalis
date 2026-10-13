import SwiftUI

/// The unit classes (Date & Time, Bits & Bytes, Length, the Earth distance…); a class opens its
/// units, on two tabs (Metric / Imperial, Decimal / Binary…); a unit is added to the request.
struct MeasuresView: View {
    @EnvironmentObject private var model: AppModel
    @State private var classes: [UnitClass] = []

    var body: some View {
        NavigationStack {
            List {
                ForEach(classes) { c in
                    NavigationLink(value: c) {
                        VStack(alignment: .leading) {
                            Text(c.name)
                            Text(c.units.map(\.symbol).joined(separator: " "))
                                .font(.footnote.monospaced())
                                .foregroundStyle(.secondary)
                                .lineLimit(1)
                        }
                    }
                    // The distance along the Earth's surface, after Length.
                    if c.name == "Length" {
                        Button {
                            model.use("distance ")
                        } label: {
                            VStack(alignment: .leading) {
                                Text("Earth distance").foregroundStyle(.primary)
                                Text("Rome to Paris, 41.9,12.5 to Tokyo").font(.footnote).foregroundStyle(.secondary)
                            }
                        }
                    }
                }
            }
            .navigationTitle("Measures")
            .navigationDestination(for: UnitClass.self) { UnitsView(unitClass: $0) }
            .task { if classes.isEmpty { classes = await Task.detached { Engine.measures() }.value } }
        }
    }
}

struct UnitsView: View {
    @EnvironmentObject private var model: AppModel
    let unitClass: UnitClass
    @State private var tab = 0

    /// A tab's units: the first holds the metric ones, the second the imperial ones; both, those
    /// for both.
    private func units(_ tab: Int) -> [UnitItem] {
        unitClass.units.filter { $0.system == "both" || $0.system == (tab == 0 ? "metric" : "imperial") }
    }

    private var tabs: [Int] { [0, 1].filter { !units($0).isEmpty } }

    var body: some View {
        List {
            if tabs.count > 1 {
                Picker("System", selection: $tab) {
                    ForEach(tabs, id: \.self) { Text(unitClass.systems[$0]).tag($0) }
                }
                .pickerStyle(.segmented)
            }
            ForEach(units(tabs.contains(tab) ? tab : (tabs.first ?? 0))) { unit in
                Button {
                    model.add(unit.symbol)
                } label: {
                    HStack {
                        Text(unit.symbol).font(.body.monospaced()).foregroundStyle(.primary)
                        Spacer()
                        Text(unit.name).foregroundStyle(.secondary)
                    }
                }
            }
        }
        .navigationTitle(unitClass.name)
    }
}

/// Every currency, searched by code, name or symbol; one is added to the request.
struct CurrenciesView: View {
    @EnvironmentObject private var model: AppModel
    @State private var all: [Currency] = []
    @State private var search = ""

    private var found: [Currency] {
        let q = search.trimmingCharacters(in: .whitespaces).lowercased()
        guard !q.isEmpty else { return all }
        return all.filter { c in
            c.code.lowercased().hasPrefix(q) || c.name.lowercased().contains(q) || c.symbols.contains { $0.lowercased() == q }
        }
    }

    var body: some View {
        NavigationStack {
            List(found) { c in
                Button {
                    model.add(c.code)
                } label: {
                    HStack {
                        Text(c.code).font(.body.monospaced()).foregroundStyle(.primary)
                        Text(c.symbols.first ?? "").foregroundStyle(.secondary).frame(minWidth: 32, alignment: .leading)
                        Text(c.name).foregroundStyle(.secondary)
                    }
                }
            }
            .searchable(text: $search, prompt: "Code, name or symbol")
            .navigationTitle("Currencies")
            .task { if all.isEmpty { all = await Task.detached { Engine.currencies() }.value } }
        }
    }
}

/// Useful values by section; an entry opens its details, and its value goes into the request.
struct TriviaView: View {
    @EnvironmentObject private var model: AppModel
    @State private var sections: [TriviaSection] = []

    var body: some View {
        NavigationStack {
            List {
                ForEach(sections) { section in
                    Section(section.name) {
                        ForEach(section.entries) { entry in
                            NavigationLink {
                                TriviaDetail(entry: entry)
                            } label: {
                                VStack(alignment: .leading) {
                                    Text(entry.name)
                                    Text(entry.value).font(.footnote.monospaced()).foregroundStyle(.secondary)
                                }
                            }
                        }
                    }
                }
            }
            .navigationTitle("Trivia")
            // Live values change: read again each time.
            .task { sections = await Task.detached { Engine.trivia() }.value }
        }
    }
}

struct TriviaDetail: View {
    @EnvironmentObject private var model: AppModel
    let entry: TriviaEntry

    var body: some View {
        List {
            Section { Text(entry.description) }
            Section {
                ForEach(entry.popup, id: \.self) { line in
                    VStack(alignment: .leading, spacing: 2) {
                        Text(line.first ?? "").font(.footnote.bold())
                        Text(line.dropFirst().first ?? "").textSelection(.enabled)
                    }
                }
            }
            if !entry.written.isEmpty {
                Button("Use in a request") { model.use(entry.written) }
            }
        }
        .navigationTitle(entry.name)
    }
}
