import SwiftUI

struct CalendarView: View {
    @State private var season = "current"
    @State private var state: Loadable<[Race]> = .loading
    @State private var winners: [String: RaceResult] = [:]

    var body: some View {
        List {
            Section { SeasonPicker(season: $season) }
            switch state {
            case .loading:
                Section { ProgressView().frame(maxWidth: .infinity) }
            case .failed(let message):
                Section {
                    Text(message).foregroundStyle(.secondary)
                    Button(L("Réessayer", "Retry")) { Task { await load() } }
                }
            case .loaded(let races):
                let nextId = races.first { !$0.isOver() }?.id
                Section {
                    ForEach(races) { race in
                        NavigationLink(value: race) {
                            CalendarRow(race: race, isNext: race.id == nextId, winner: winners[race.round])
                        }
                        .listRowBackground(race.id == nextId ? Color.f1Red.opacity(0.18) : nil)
                    }
                }
            }
        }
        .listStyle(.insetGrouped)
        .navigationTitle(L("Calendrier", "Calendar"))
        .f1Destinations()
        .refreshable {
            await F1API.shared.clearCache()
            await load()
        }
        .task(id: season) { await load() }
    }

    private func load() async {
        do {
            async let w = try? F1API.shared.winners(season)
            let races = try await F1API.shared.schedule(season: season)
            state = .loaded(races)
            var map: [String: RaceResult] = [:]
            for r in await w ?? [] { if let first = r.results?.first { map[r.round] = first } }
            winners = map
        } catch {
            state = .failed(loadErrorMessage(error))
        }
    }
}

private struct CalendarRow: View {
    let race: Race
    let isNext: Bool
    let winner: RaceResult?

    var body: some View {
        HStack(spacing: 12) {
            Text("R\(race.round)")
                .font(.subheadline.weight(.heavy))
                .foregroundStyle(.secondary)
                .frame(width: 44, height: 44)
                .background(Color(uiColor: .tertiarySystemBackground), in: RoundedRectangle(cornerRadius: 10))
            VStack(alignment: .leading, spacing: 2) {
                Text("\(Flag.country(race.circuit.location.country)) \(race.raceName)")
                    .fontWeight(.bold)
                    .fixedSize(horizontal: false, vertical: true)
                HStack(spacing: 6) {
                    if let start = race.start { Text(start.f1Day) }
                    if race.isSprintWeekend {
                        Text("SPRINT")
                            .font(.caption2.bold())
                            .padding(.horizontal, 6)
                            .padding(.vertical, 1)
                            .background(Color.f1Red.opacity(0.2), in: RoundedRectangle(cornerRadius: 6))
                            .foregroundStyle(Color.f1Red)
                    }
                }
                .font(.footnote)
                .foregroundStyle(.secondary)
                if let w = winner {
                    Text("🏆 \(w.driver.fullName) · \(w.constructor.name)")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            if isNext {
                Text(L("Prochain", "Next"))
                    .font(.caption2.bold())
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(Color.f1Red, in: Capsule())
                    .foregroundStyle(.white)
            }
        }
        .opacity(race.isOver() && winner == nil ? 0.6 : 1)
    }
}
