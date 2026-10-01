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

/// Barre d'onglets du bas, comme sur le site. Toute la navigation est verticale :
/// aucune vue ne défile horizontalement.
struct RootView: View {
    var body: some View {
        TabView {
            NavigationStack { HomeView() }
                .tabItem { Label(L("Accueil", "Home"), systemImage: "house.fill") }
            NavigationStack { LiveView() }
                .tabItem { Label(L("Direct", "Live"), systemImage: "dot.radiowaves.left.and.right") }
            NavigationStack { CalendarView() }
                .tabItem { Label(L("Calendrier", "Calendar"), systemImage: "calendar") }
            NavigationStack { StandingsView() }
                .tabItem { Label(L("Classements", "Standings"), systemImage: "trophy.fill") }
            NavigationStack { ExplorerView() }
                .tabItem { Label(L("Explorer", "Explore"), systemImage: "archivebox.fill") }
        }
    }
}
