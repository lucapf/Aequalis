import SwiftUI
import TallyCore

/// The panel under the menu bar icon: the header (title, online or offline, the TUI), the field,
/// the answer, the choices of an ambiguous request, Recent; or About.
struct PanelView: View {
    @ObservedObject var model: PanelModel
    let close: () -> Void
    @FocusState private var focused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            header
            if model.showAbout {
                AboutView(model: model)
            } else {
                calculator
            }
        }
        .padding(14)
        .frame(width: 440)
        .onExitCommand(perform: close)
    }

    private var header: some View {
        HStack(spacing: 8) {
            Button {
                model.showAbout.toggle()
            } label: {
                Text("Tally").font(.headline)
            }
            .buttonStyle(.plain)
            .help(model.showAbout ? "Back to the calculator" : "About Tally")
            Spacer()
            Circle()
                .fill(model.status == "online" ? Color.green : Color.secondary)
                .frame(width: 7, height: 7)
            Text(model.status)
                .font(.caption)
                .foregroundStyle(.secondary)
                .help(model.status.contains("forced") ? "Offline in Settings (TALLY_MODE=offline)" : "The last online sync")
            Button {
                model.openTui()
                close()
            } label: {
                Label("TUI", systemImage: "terminal")
            }
            .controlSize(.small)
            .disabled(model.tally == nil)
            .help("The full interactive mode, in Terminal")
        }
    }

    private var calculator: some View {
        VStack(alignment: .leading, spacing: 8) {
            TextField("10 km mi · 12+4/2 · 100 usd eur · now Tokyo", text: $model.query)
                .textFieldStyle(.roundedBorder)
                .font(.system(.body, design: .monospaced))
                .focused($focused)
                .onSubmit { model.commit() }
                .onChange(of: model.focusToken) { _ in focused = true }
                .onAppear { focused = true }

            if model.missing {
                Text("The `tally` command was not found: install it in ~/.local/bin or /opt/homebrew/bin, or set its path in About.")
                    .font(.callout)
                    .foregroundStyle(.secondary)
            } else if let reply = model.reply {
                answer(reply)
            }

            if !model.copied.isEmpty {
                Text("Copied \(model.copied)")
                    .font(.caption)
                    .foregroundStyle(Color.accentColor)
            }

            if !model.recent.isEmpty {
                Divider()
                Text("Recent").font(.caption).foregroundStyle(.secondary)
                ForEach(model.recent, id: \.self) { entry in
                    Button {
                        model.use(entry.input)
                    } label: {
                        Text("\(entry.input)   =   \(entry.result)")
                            .font(.system(.callout, design: .monospaced))
                            .lineLimit(1)
                            .truncationMode(.middle)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                }
            }

            Text("Enter copies · Esc closes · ⌃⌥T opens it anywhere · click Tally for About")
                .font(.caption2)
                .foregroundStyle(.secondary)
        }
    }

    @ViewBuilder
    private func answer(_ reply: Reply) -> some View {
        HStack(alignment: .firstTextBaseline) {
            Text(reply.headline)
                .font(reply.ok ? .title3 : .callout)
                .foregroundStyle(reply.ok ? Color.primary : Color.secondary)
                .textSelection(.enabled)
                .fixedSize(horizontal: false, vertical: true)
            Spacer()
            if reply.ok {
                Button("Copy") { model.commit() }
                    .controlSize(.small)
            }
        }
        ForEach(reply.remarks, id: \.self) { remark in
            Text(remark).font(.caption).foregroundStyle(.secondary)
        }
        // An ambiguous date, a misspelt place, a country with several time zones: one button per
        // reading, which writes its request into the field.
        if let ambiguous = reply.ambiguous {
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 180), alignment: .leading)], alignment: .leading, spacing: 6) {
                ForEach(ambiguous.choices, id: \.self) { choice in
                    Button(choice.label) { model.use(choice.request) }
                }
            }
        }
    }
}

/// About: the logo and the facts from `tally --about`, the memory, and the settings.
struct AboutView: View {
    @ObservedObject var model: PanelModel

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(model.about)
                .font(.system(size: 11, design: .monospaced))
                .textSelection(.enabled)
                .fixedSize(horizontal: false, vertical: true)
            if !model.memory.isEmpty {
                Text("Memory · \(model.memory)").font(.caption).foregroundStyle(.secondary)
            }
            Divider()
            Toggle("Offline: never use the network (TALLY_MODE=offline)", isOn: $model.offline)
            Toggle("Open at login", isOn: Binding(get: { model.launchAtLogin }, set: { model.launchAtLogin = $0 }))
            HStack {
                Text("tally command")
                TextField("automatic", text: $model.command)
                    .textFieldStyle(.roundedBorder)
                    .font(.system(.body, design: .monospaced))
            }
            HStack {
                Button("Back") { model.showAbout = false }
                Spacer()
                Button("Quit Tally") { NSApp.terminate(nil) }
            }
        }
    }
}
