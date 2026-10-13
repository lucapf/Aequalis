import SwiftUI

/// Every setting of the core (yes/no, choices, typed values), saved at once; the download of the
/// online data, the notices, and About.
struct SettingsView: View {
    @EnvironmentObject private var model: AppModel
    @State private var settings: [Setting] = []
    @State private var message = ""
    @State private var about = ""

    var body: some View {
        NavigationStack {
            Form {
                Section("Online data") {
                    Button {
                        model.sync(automatic: false)
                    } label: {
                        Label(model.syncing ? "Downloading…" : "Download everything", systemImage: "arrow.down.circle")
                    }
                    .disabled(model.syncing)
                    if !model.syncMessage.isEmpty {
                        Text(model.syncMessage).font(.footnote).foregroundStyle(.secondary)
                    }
                    ForEach(model.notices, id: \.self) { Text($0).font(.footnote) }
                }
                Section {
                    ForEach(settings) { setting in
                        row(setting)
                    }
                } footer: {
                    if !message.isEmpty { Text(message) }
                }
                Section("About") {
                    Text(about).font(.caption2.monospaced()).textSelection(.enabled)
                }
            }
            .navigationTitle("Settings")
            .task { await reload() }
        }
    }

    @ViewBuilder private func row(_ s: Setting) -> some View {
        switch s.kind {
        case "bool":
            Toggle(isOn: Binding(get: { s.value == "true" }, set: { save(s.key, $0 ? "true" : "false") })) {
                title(s)
            }
        case "choice":
            Picker(selection: Binding(get: { s.value }, set: { save(s.key, $0) })) {
                ForEach(s.options ?? [], id: \.self) { Text($0).tag($0) }
            } label: {
                title(s)
            }
        default:
            TypedSetting(setting: s) { save(s.key, $0) }
        }
    }

    private func title(_ s: Setting) -> some View {
        VStack(alignment: .leading) {
            Text(s.key)
            Text(s.about).font(.caption).foregroundStyle(.secondary)
        }
    }

    /// Saves a value (nil: the default) and reads the settings again.
    private func save(_ key: String, _ value: String?) {
        Task {
            let failed = await Task.detached { Engine.set(key, value) }.value
            message = failed ?? "saved: \(key)"
            await reload()
        }
    }

    private func reload() async {
        settings = await Task.detached { Engine.settings() }.value
        about = await Task.detached { Engine.eval("--about").result ?? "" }.value
    }
}

/// A setting typed in (a duration, a format, an address): saved on Return; Default puts back
/// its default.
private struct TypedSetting: View {
    let setting: Setting
    let save: (String?) -> Void
    @State private var text = ""

    var body: some View {
        VStack(alignment: .leading) {
            HStack {
                Text(setting.key)
                Spacer()
                if setting.value != setting.default {
                    Button("Default") { save(nil) }.font(.footnote)
                }
            }
            TextField(setting.default, text: $text)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .font(.body.monospaced())
                .onSubmit { save(text) }
            Text(setting.about).font(.caption).foregroundStyle(.secondary)
        }
        .onAppear { text = setting.value }
        .onChange(of: setting.value) { _, value in text = value }
    }
}
