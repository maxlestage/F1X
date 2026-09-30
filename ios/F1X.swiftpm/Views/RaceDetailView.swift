import SwiftUI

struct RaceDetailView: View {
    let race: Race

    @State private var results: [RaceResult] = []
    @State private var sprint: [RaceResult] = []
    @State private var qualifying: [QualifyingResult] = []
    @State private var isLoading = true

    var body: some View {
        List {
            Section {
                VStack(alignment: .leading, spacing: 8) {
                    Eyebrow(text: "Manche \(race.round)")
                    Text("\(Flag.country(race.circuit.location.country)) \(race.raceName)")
                        .font(.title2.weight(.heavy))
                        .fixedSize(horizontal: false, vertical: true)
                    Text(race.circuit.circuitName).foregroundStyle(.secondary)
                    Text("\(race.circuit.location.locality), \(race.circuit.location.country)")
                        .foregroundStyle(.secondary)
                    if !race.isOver(), let start = race.start {
                        CountdownView(target: start).padding(.top, 4)
                    }
                }
                .padding(.vertical, 4)
            }

            Section("Programme") {
                SessionsList(race: race)
            }

            if !results.isEmpty {
                Section("Course") {
                    ForEach(results) { ResultRow(result: $0) }
                }
            }

            if !sprint.isEmpty {
                Section("Sprint") {
                    ForEach(sprint) { ResultRow(result: $0) }
                }
            }

            if !qualifying.isEmpty {
                Section("Qualifications") {
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

            if !isLoading && race.isOver() && results.isEmpty {
                Section {
                    Text("Les résultats ne sont pas encore disponibles.").foregroundStyle(.secondary)
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

    private func load() async {
        defer { isLoading = false }
        // Pas de résultats à chercher plus de 3 jours avant la course.
        guard let start = race.start, Date.now > start.addingTimeInterval(-3 * 86400) else { return }
        let round = race.roundNumber
        async let r = try? F1API.shared.results(round: round)
        async let s = try? F1API.shared.sprint(round: round)
        async let q = try? F1API.shared.qualifying(round: round)
        results = await r ?? []
        sprint = await s ?? []
        qualifying = await q ?? []
    }
}

private struct ResultRow: View {
    let result: RaceResult

    var body: some View {
        NavigationLink(value: result.driver) {
            StandingRow(
                position: result.positionText,
                teamId: result.constructor.constructorId,
                title: result.hasFastestLap
                    ? Text("\(driverTitle(result.driver))  \(Text("⏱").foregroundStyle(Color.f1Purple))")
                    : driverTitle(result.driver),
                subtitle: [result.constructor.name, result.outcome].filter { !$0.isEmpty }.joined(separator: " · ")
            ) {
                if result.scoredPoints {
                    PointsLabel(value: "+\(result.points)", suffix: "")
                }
            }
        }
    }
}
