import SwiftUI
import WidgetKit

@main
struct F1XApp: App {
    @Environment(\.scenePhase) private var scenePhase

    var body: some Scene {
        WindowGroup {
            RootView()
                .preferredColorScheme(.dark)
                .tint(.f1Red)
        }
        // En quittant l'app, le widget se recharge avec les derniers résultats.
        .onChange(of: scenePhase) { _, phase in
            if phase == .background { WidgetCenter.shared.reloadAllTimelines() }
        }
    }
}

/// Barre d'onglets du bas, comme sur le site. Toute la navigation est verticale :
/// aucune vue ne défile horizontalement. Écran de démarrage animé, logo de rubrique
/// animé à chaque changement d'onglet et de page.
struct RootView: View {
    @State private var tab: AppSection = .home
    @State private var intro: AppSection?
    @State private var splash = true
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        ZStack {
            TabView(selection: $tab) {
                stack(.home) { HomeView() }
                stack(.live) { LiveView() }
                stack(.calendar) { CalendarView() }
                stack(.standings) { StandingsView() }
                stack(.explore) { ExplorerView() }
            }
            if let intro {
                SectionIntro(section: intro)
                    .id(intro)
                    .transition(.opacity)
            }
            if splash && !reduceMotion {
                SplashView { withAnimation(.easeIn(duration: 0.15)) { splash = false } }
                    .transition(.opacity)
                    .zIndex(10)
            }
        }
        .onChange(of: tab) { _, new in
            guard !reduceMotion else { return }
            withAnimation(.easeOut(duration: 0.15)) { intro = new }
            Task {
                try? await Task.sleep(nanoseconds: 750_000_000)
                if intro == new { withAnimation(.easeIn(duration: 0.25)) { intro = nil } }
            }
        }
    }

    private func stack<Content: View>(_ section: AppSection, @ViewBuilder content: () -> Content) -> some View {
        NavigationStack { content().sectionLogo() }
            .environment(\.appSection, section)
            .tabItem { Label(section.label, systemImage: section.symbol) }
            .tag(section)
    }
}
