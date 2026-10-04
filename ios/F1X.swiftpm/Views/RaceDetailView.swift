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

            // Tout savoir sur la course, juste sous le titre (pas caché au fond de la page).
            if race.isOver(), let year = Int(race.season), year >= 2023 {
                FoldSection(L("Tout savoir sur la course", "Everything about the race"),
                            footer: L("Fiche de chaque pilote tour par tour, secteurs, écarts, pneus, arrêts, télémétrie, direction de course et radios.",
                                      "Each driver's lap-by-lap file, sectors, gaps, tyres, stops, telemetry, race control and radio.")) {
                    RaceDataLinks(year: year, date: race.date)
                        .listRowInsets(EdgeInsets(top: 12, leading: 12, bottom: 12, trailing: 12))
                }
            }

            // Replay automatique de la course (vraies positions des pilotes).
            if race.isOver(), let year = Int(race.season), year >= 2023 {
                FoldSection(L("Replay : course, qualifs, sprint", "Replay: race, quali, sprint"),
                            footer: L("Démarre tout seul à ×30. Positions de chaque pilote d'après son avancement dans le tour (données OpenF1).",
                                      "Starts on its own at ×30. Each driver's position from their progress through the lap (OpenF1 data).")) {
                    RaceReplayCard(year: year, date: race.date)
                }
            }

            FoldSection(L("Programme", "Schedule")) {
                SessionsList(race: race)
            }

            if !race.isOver() {
                Section {
                    WeatherCard(race: race)
                        .listRowInsets(EdgeInsets())
                        .listRowBackground(Color.clear)
                }
            }

            FoldSection(L("Le circuit", "The circuit")) {
                TrackPanel(circuitId: race.circuit.circuitId)
                if let lat = race.circuit.location.lat.flatMap(Double.init), let lon = race.circuit.location.long.flatMap(Double.init) {
                    CircuitMapView(name: race.circuit.circuitName, lat: lat, lon: lon)
                }
                NavigationLink(value: race.circuit) { Text(L("Fiche complète du circuit", "Full circuit details")) }
            }

            if race.isOver(), let year = Int(race.season), year >= 2023 {
                FoldSection(L("Données OpenF1", "OpenF1 data")) {
                    MeetingLinkButton(year: year, date: race.date)
                }
            }

            if !results.isEmpty {
                FoldSection(L("Course", "Race")) {
                    ForEach(Array(results.enumerated()), id: \.offset) { ResultRow(result: $0.element) }
                }
            }

            if !sprint.isEmpty {
                FoldSection("Sprint") {
                    ForEach(Array(sprint.enumerated()), id: \.offset) { ResultRow(result: $0.element) }
                }
            }

            if !qualifying.isEmpty {
                FoldSection(L("Qualifications", "Qualifying")) {
                    ForEach(Array(qualifying.enumerated()), id: \.offset) { _, q in
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

            if race.isOver(), (Int(race.season) ?? 0) >= 2023 {
                PitDetailSection(query: "year=\(race.season)&date=\(race.date)")
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
        .autoRefresh { await load() }
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
