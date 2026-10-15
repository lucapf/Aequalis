import SwiftUI

/// The request, its answer (the result with Copy, or what is wrong, or the choices), the notices
/// and Recent; operators above the keyboard.
struct CalculateView: View {
    @EnvironmentObject private var model: AppModel
    @FocusState private var focused: Bool

    /// The keys above the keyboard: the ones the text keyboard hides behind its other pages.
    private let keys = ["+", "-", "*", "/", "^", "(", ")", "%", "."]

    var body: some View {
        NavigationStack {
            List {
                Section {
                    TextField("10 km mi · 12+4/2 · Rome Paris", text: $model.query)
                        .font(.title3.monospaced())
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .submitLabel(.done)
                        .focused($focused)
                        .onSubmit { model.submit() }
                    answer
                }
                choices
                notices
                recent
            }
            .navigationTitle("Tally")
            .toolbar {
                ToolbarItemGroup(placement: .keyboard) {
                    ForEach(keys, id: \.self) { key in
                        Button(key) { model.query += key }
                            .font(.body.monospaced())
                    }
                }
            }
            .onAppear { focused = true }
        }
    }

    @ViewBuilder private var answer: some View {
        if let reply = model.reply, !model.quiet {
            HStack(alignment: .firstTextBaseline) {
                Text(reply.headline)
                    .font(reply.ok ? Font.title2.monospaced() : Font.body)
                    .foregroundStyle(reply.ok ? Color.primary : Color.secondary)
                    .textSelection(.enabled)
                Spacer()
                if reply.ok {
                    Button {
                        model.submit()
                    } label: {
                        Label(model.copied.isEmpty ? "Copy" : "Copied", systemImage: "doc.on.doc")
                    }
                    .buttonStyle(.bordered)
                }
            }
            ForEach(reply.remarks, id: \.self) { remark in
                Text(remark).font(.footnote).foregroundStyle(.secondary)
            }
        }
    }

    /// An ambiguous request: one button per reading; the chosen one is written in.
    @ViewBuilder private var choices: some View {
        if let a = model.reply?.ambiguous, !model.quiet {
            Section("Which one?") {
                ForEach(a.choices, id: \.self) { choice in
                    Button {
                        model.use(choice.request)
                    } label: {
                        VStack(alignment: .leading) {
                            Text(choice.pattern ?? choice.request)
                            if let date = choice.date, !date.isEmpty {
                                Text(date).font(.footnote).foregroundStyle(.secondary)
                            }
                        }
                    }
                }
            }
        }
    }

    @ViewBuilder private var notices: some View {
        if !model.notices.isEmpty {
            Section {
                ForEach(model.notices, id: \.self) { notice in
                    Button {
                        model.tab = .settings
                    } label: {
                        Label(notice, systemImage: "arrow.down.circle")
                    }
                }
            }
        }
    }

    @ViewBuilder private var recent: some View {
        if !model.recent.isEmpty {
            Section {
                ForEach(model.recent, id: \.self) { entry in
                    Button {
                        model.use(entry.input)
                    } label: {
                        VStack(alignment: .leading) {
                            Text(entry.input).foregroundStyle(.primary)
                            Text("= " + entry.result).font(.footnote.monospaced()).foregroundStyle(.secondary)
                        }
                    }
                }
            } header: {
                HStack {
                    Text("Recent")
                    Spacer()
                    Button("Clear") { model.clearRecent() }.font(.footnote)
                }
            }
        }
    }
}
