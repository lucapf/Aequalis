import BackgroundTasks
import SwiftUI

/// The background refresh of the online data (rates, live values, the city archive's check, as
/// the refresh settings say); listed in Info.plist (BGTaskSchedulerPermittedIdentifiers).
let refreshTask = "com.lucapf.tally.refresh"

@main
struct TallyApp: App {
    @StateObject private var model = AppModel()
    @Environment(\.scenePhase) private var phase

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(model)
        }
        .onChange(of: phase) { _, now in
            if now == .active { model.sync(automatic: true) }
            if now == .background { scheduleRefresh() }
        }
        .backgroundTask(.appRefresh(refreshTask)) {
            _ = Engine.sync(automatic: true)
            scheduleRefresh()
        }
    }
}

/// Asks iOS for the next background refresh, in about 6 hours at the earliest.
func scheduleRefresh() {
    let request = BGAppRefreshTaskRequest(identifier: refreshTask)
    request.earliestBeginDate = Date(timeIntervalSinceNow: 6 * 3600)
    try? BGTaskScheduler.shared.submit(request)
}

struct ContentView: View {
    @EnvironmentObject private var model: AppModel

    var body: some View {
        TabView(selection: $model.tab) {
            CalculateView()
                .tabItem { Label("Tally", systemImage: "equal.square") }
                .tag(Tab.calculate)
            MeasuresView()
                .tabItem { Label("Measures", systemImage: "ruler") }
                .tag(Tab.measures)
            CurrenciesView()
                .tabItem { Label("Currencies", systemImage: "banknote") }
                .tag(Tab.currencies)
            TriviaView()
                .tabItem { Label("Trivia", systemImage: "sparkles") }
                .tag(Tab.trivia)
            SettingsView()
                .tabItem { Label("Settings", systemImage: "gearshape") }
                .tag(Tab.settings)
        }
    }
}
