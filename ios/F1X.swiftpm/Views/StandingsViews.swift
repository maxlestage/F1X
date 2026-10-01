import SwiftUI

/// Sélecteur de saison (1950 → saison en cours).
struct SeasonPicker: View {
    @Binding var season: String
    @State private var seasons: [String] = []

    var body: some View {
        Picker(L("Saison", "Season"), selection: $season) {
            Text(L("Saison en cours", "Current season")).tag("current")
            ForEach(seasons.reversed(), id: \.self) { Text($0).tag($0) }
        }
        .task { if seasons.isEmpty { seasons = (try? await F1API.shared.seasons()) ?? [] } }
    }
}

/// Classements pilotes et écuries, pour n'importe quelle saison.
struct StandingsView: View {
    @State private var season = "current"
    @State private var teams = false
    @State private var drivers: Loadable<[DriverStanding]> = .loading
    @State private var constructors: [ConstructorStanding] = []

    var body: some View {
        List {
            Section {
                SeasonPicker(season: $season)
                Picker("", selection: $teams) {
                    Text(L("Pilotes", "Drivers")).tag(false)
                    Text(L("Écuries", "Teams")).tag(true)
                }
                .pickerStyle(.segmented)
            }
            switch drivers {
            case .loading:
                Section { ProgressView().frame(maxWidth: .infinity) }
            case .failed(let message):
                Section {
                    Text(message).foregroundStyle(.secondary)
                    Button(L("Réessayer", "Retry")) { Task { await load() } }
                }
            case .loaded(let list):
                if teams {
                    let leader = constructors.first.flatMap { Double($0.points) } ?? 0
                    Section {
                        ForEach(constructors) { s in
                            NavigationLink(value: s.constructor) {
                                VStack(alignment: .leading, spacing: 6) {
                                    StandingRow(position: s.rank, teamId: s.constructor.constructorId,
                                                title: Text(s.constructor.name).bold(), subtitle: winsLabel(s.wins)) {
                                        PointsLabel(value: s.points)
                                    }
                                    ProgressView(value: leader > 0 ? min((Double(s.points) ?? 0) / leader, 1) : 0)
                                        .tint(Team.color(s.constructor.constructorId))
                                }
                            }
                        }
                    }
                } else {
                    Section {
                        ForEach(list) { s in
                            NavigationLink(value: s.driver) {
                                StandingRow(
                                    position: s.rank,
                                    teamId: s.team?.constructorId,
                                    title: driverTitle(s.driver, flag: true),
                                    subtitle: [s.team?.name ?? "", s.wins == "0" ? "" : L("\(s.wins) V", "\(s.wins) W")]
                                        .filter { !$0.isEmpty }.joined(separator: " · "),
                                    avatar: s.driver
                                ) { PointsLabel(value: s.points) }
                            }
                        }
                    }
                }
            }
        }
        .listStyle(.insetGrouped)
        .navigationTitle(L("Classements", "Standings"))
        .f1Destinations()
        .refreshable {
            await F1API.shared.clearCache()
            await load()
        }
        .task(id: season) { await load() }
    }

    private func load() async {
        do {
            async let c = try? F1API.shared.constructorStandings(season: season)
            let d = try await F1API.shared.driverStandings(season: season)
            constructors = await c ?? []
            drivers = .loaded(d)
        } catch {
            drivers = .failed(loadErrorMessage(error))
        }
    }
}
