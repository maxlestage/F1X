import SwiftUI

@main
struct F1XApp: App {
    var body: some Scene {
        WindowGroup {
            RootView()
                .preferredColorScheme(.dark)
                .tint(.f1Red)
        }
    }
}

/// Barre d'onglets du bas. Toute la navigation est verticale : aucune vue ne défile horizontalement.
struct RootView: View {
    var body: some View {
        TabView {
            NavigationStack { HomeView() }
                .tabItem { Label("Accueil", systemImage: "house.fill") }
            NavigationStack { CalendarView() }
                .tabItem { Label("Calendrier", systemImage: "calendar") }
            NavigationStack { DriverStandingsView() }
                .tabItem { Label("Pilotes", systemImage: "person.fill") }
            NavigationStack { ConstructorStandingsView() }
                .tabItem { Label("Écuries", systemImage: "flag.checkered") }
        }
    }
}
