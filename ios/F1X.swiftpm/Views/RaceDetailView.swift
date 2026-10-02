import SwiftUI

struct RaceDetailView: View {
    let race: Race

    @State private var results: [RaceResult] = []
    @State private var sprint: [RaceResult] = []
    @State private var qualifying: [QualifyingResult] = []
    @State private var pits: [PitStop] = []
    @State private var isLoading = true

    var body: some View {
        List {
            Section {
                VStack(alignment: .leading, spacing: 8) {
                    Eyebrow(text: "\(race.season) · \(L("Manche", "Round")) \(race.round)")
                    Text("\(Flag.country(race.circuit.location.country)) \(race.raceName)")
                        .font(.title2.weight(.heavy))
                        .fixedSize(horizontal: false, vertical: true)
                    NavigationLink(value: race.circuit) {
                        Text(race.circuit.circuitName).foregroundStyle(Color.f1Red)
                    }
                    Text("\(race.circuit.location.locality), \(race.circuit.location.country)")
                        .foregroundStyle(.secondary)
                    if !race.isOver(), let start = race.start {
                        CountdownView(target: start).padding(.top, 4)
                    }
                }
                .padding(.vertical, 4)
            }

            Section(L("Programme", "Schedule")) {
                SessionsList(race: race)
            }

            if !race.isOver() {
                Section {
                    WeatherCard(race: race)
                        .listRowInsets(EdgeInsets())
                        .listRowBackground(Color.clear)
                }
            }

            Section(L("Le circuit", "The circuit")) {
                TrackPanel(circuitId: race.circuit.circuitId)
                NavigationLink(value: race.circuit) { Text(L("Fiche complète du circuit", "Full circuit details")) }
            }

            if race.isOver(), let year = Int(race.season), year >= 2023 {
                Section(L("Données OpenF1", "OpenF1 data")) {
                    MeetingLinkButton(year: year, date: race.date)
                }
            }

            if !results.isEmpty {
                Section(L("Course", "Race")) {
                    ForEach(results) { ResultRow(result: $0) }
                }
            }

            if !sprint.isEmpty {
                Section("Sprint") {
                    ForEach(sprint) { ResultRow(result: $0) }
                }
            }

            if !qualifying.isEmpty {
                Section(L("Qualifications", "Qualifying")) {
                    ForEach(qualifying) { q in
                        NavigationLink(value: q.driver) {
                            StandingRow(
                                position: q.position,
                                teamId: q.constructor.constructorId,
                                title: driverTitle(q.driver),
                                subtitle: q.constructor.name
                            ) {
                                if let best = q.best {
                                    PointsLabel(value: best.time, suffix: best.segment)
                                }
                            }
                        }
                    }
                }
            }

            FastestLapsSection(results: results)

            if !pits.isEmpty {
                PitStopsSection(pits: pits, results: results)
            }

            if race.isOver(), (Int(race.season) ?? 0) >= 1996, !results.isEmpty {
                LapByLapSection(season: race.season, round: race.roundNumber, results: results, pits: pits)
            }

            RaceSummarySection(results: results)

            if !isLoading && race.isOver() && results.isEmpty {
                Section {
                    Text(L("Les résultats ne sont pas encore disponibles.", "Results are not available yet.")).foregroundStyle(.secondary)
                }
            }
        }
        .listStyle(.insetGrouped)
        .navigationTitle(race.raceName)
        .navigationBarTitleDisplayMode(.inline)
        .overlay { if isLoading { ProgressView() } }
        .task { await load() }
        .refreshable {
            await F1API.shared.clearCache()
            await load()
        }
    }

    private func name(_ driverId: String) -> String {
        (results + sprint).first { $0.driver.driverId == driverId }?.driver.familyName ?? driverId.capitalized
    }

    private func load() async {
        defer { isLoading = false }
        // Pas de résultats à chercher plus de 3 jours avant la course.
        guard let start = race.start, Date.now > start.addingTimeInterval(-3 * 86400) else { return }
        let round = race.roundNumber, season = race.season
        async let r = try? F1API.shared.results(season: season, round: round)
        async let s = try? F1API.shared.sprint(season: season, round: round)
        async let q = try? F1API.shared.qualifying(season: season, round: round)
        async let p = try? F1API.shared.pitStops(season: season, round: round)
        results = await r ?? []
        sprint = await s ?? []
        qualifying = await q ?? []
        pits = await p ?? []
    }
}

struct ResultRow: View {
    let result: RaceResult

    var body: some View {
        NavigationLink(value: result.driver) {
            StandingRow(
                position: result.positionText,
                teamId: result.constructor.constructorId,
                title: result.hasFastestLap
                    ? Text("\(driverTitle(result.driver))  \(Text("⏱").foregroundStyle(Color.f1Purple))")
                    : driverTitle(result.driver),
                subtitle: [result.constructor.name, gained, result.outcome].filter { !$0.isEmpty }.joined(separator: " · "),
                avatar: result.driver
            ) {
                if result.scoredPoints {
                    PointsLabel(value: "+\(result.points)", suffix: "")
                }
            }
        }
    }

    /// Places gagnées ou perdues depuis la grille.
    private var gained: String {
        guard let g = result.grid.flatMap(Int.init), g > 0, let p = Int(result.position) else { return "" }
        let d = g - p
        return d > 0 ? "▲\(d)" : (d < 0 ? "▼\(-d)" : "")
    }
}
